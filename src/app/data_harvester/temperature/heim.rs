//! Gets temperature data via heim.

use super::{is_temp_filtered, temp_vec_sort, TempHarvest, TemperatureType};
use crate::app::Filter;

/// Get temperature sensors from the linux sysfs interface `/sys/class/hwmon`
///
/// This method will return `0` as the temperature for devices, such as GPUs,
/// that support power management features and power themselves off.
///
/// Specifically, in laptops with iGPUs and dGPUs, if the dGPU is capable of
/// entering ACPI D3cold, reading the temperature sensors will wake it,
/// and keep it awake, wasting power.
///
/// For such devices, this method will only query the sensors IF the
/// device is already in ACPI D0
///
/// This has the notable issue that once this happens,
/// the device will be *kept* on through the sensor reading,
/// and not be able to re-enter ACPI D3cold.
pub async fn get_temperature_data(
    temp_type: &TemperatureType, actually_get: bool, filter: &Option<Filter>,
) -> crate::utils::error::Result<Option<Vec<TempHarvest>>> {
    use std::{fs, path::Path};

    if !actually_get {
        return Ok(None);
    }

    let mut temperature_vec: Vec<TempHarvest> = Vec::new();

    // Documented at https://www.kernel.org/doc/Documentation/ABI/testing/sysfs-class-hwmon
    let path = Path::new("/sys/class/hwmon");

    // NOTE: Technically none of this is async, *but* sysfs is in memory,
    // so in theory none of this should block if we're slightly careful.
    // Of note is that reading the temperature sensors of a device that has
    // `/sys/class/hwmon/hwmon*/device/d3cold_allowed` can potentially
    // wake the device up, waiting to return data until it is.
    //
    // Reading the `d3cold_allowed`, `power_state`, or `tempY_label` properties
    // will not wake the device, and thus not block.
    //
    // It would probably be more ideal to use a proper async runtime..
    for entry in path.read_dir()? {
        let file = entry?;
        let path = file.path();
        // hwmon includes many sensors, we only want ones with at least one temperature sensor
        // Reading this file will wake the device, but we're only checking existence.
        let has_temp = path.join("temp1_input").exists();
        let hwmon_name = path.join("name");
        let hwmon_name = Some(fs::read_to_string(&hwmon_name)?);

        // Skip ones without temperature sensors early
        if !has_temp {
            continue;
        }

        // Whether the temperature should *actually* be read during enumeration
        // Set to false if the device is in ACPI D3cold
        let mut should_read_temp = true;
        // Documented at https://www.kernel.org/doc/Documentation/ABI/testing/sysfs-devices-power_state
        let power_state = path.join("device").join("power_state");
        if power_state.exists() {
            should_read_temp = fs::read_to_string(power_state)?.trim() == "D0";
        }

        // Enumerate the devices temperature sensors
        for entry in path.read_dir()? {
            let file = entry?;
            let name = file.file_name();
            // This should always be ASCII
            let name = name.to_str().unwrap();
            // We only want temperature sensors, skip others early
            if !(name.starts_with("temp") && name.ends_with("input")) {
                continue;
            }
            let temp = file.path();
            let temp_label = path.join(name.replace("input", "label"));
            let temp_label = fs::read_to_string(temp_label).ok();

            let name = match (&hwmon_name, &temp_label) {
                (Some(name), Some(label)) => format!("{}: {}", name.trim(), label.trim()),
                (None, Some(label)) => label.to_string(),
                (Some(name), None) => name.to_string(),
                (None, None) => String::default(),
            };

            if is_temp_filtered(filter, &name) {
                use heim::units::{thermodynamic_temperature, ThermodynamicTemperature};
                let temp = if should_read_temp {
                    let temp = fs::read_to_string(temp)?;
                    let temp = temp.trim_end().parse::<f32>().map_err(|e| {
                        crate::utils::error::BottomError::ConversionError(e.to_string())
                    })?;
                    temp / 1_000.0
                } else {
                    0.0
                };
                let temp = ThermodynamicTemperature::new::<thermodynamic_temperature::degree_celsius>(
                    temp,
                );

                temperature_vec.push(TempHarvest {
                    name,
                    temperature: match temp_type {
                        TemperatureType::Celsius => {
                            temp.get::<thermodynamic_temperature::degree_celsius>()
                        }
                        TemperatureType::Kelvin => temp.get::<thermodynamic_temperature::kelvin>(),
                        TemperatureType::Fahrenheit => {
                            temp.get::<thermodynamic_temperature::degree_fahrenheit>()
                        }
                    },
                });
            }
        }
    }

    #[cfg(feature = "nvidia")]
    {
        super::nvidia::add_nvidia_data(&mut temperature_vec, temp_type, filter)?;
    }

    temp_vec_sort(&mut temperature_vec);
    Ok(Some(temperature_vec))
}
