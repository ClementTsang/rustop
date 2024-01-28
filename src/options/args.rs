//! Argument parsing via clap.
//!
//! Note that you probably want to keep this as a single file so the build script doesn't
//! trip all over itself.

// TODO: New sections are misaligned! See if we can get that fixed.

use std::cmp::Ordering;

use clap::*;
use indoc::indoc;
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum StringOrNum {
    String(String),
    Num(u64),
}

impl From<&str> for StringOrNum {
    fn from(value: &str) -> Self {
        match value.parse::<u64>() {
            Ok(parsed) => StringOrNum::Num(parsed),
            Err(_) => StringOrNum::String(value.to_owned()),
        }
    }
}

impl From<u64> for StringOrNum {
    fn from(value: u64) -> Self {
        StringOrNum::Num(value)
    }
}

/// Returns an [`Ordering`] for two [`Arg`] values.
///
/// Note this assumes that _both have a long_ name, and will
/// panic if either are missing!
fn sort_args(a: &Arg, b: &Arg) -> Ordering {
    let a = a.get_long().unwrap();
    let b = b.get_long().unwrap();

    a.cmp(b)
}

/// Create an array of [`Arg`] values. If there is more than one value, then
/// they will be sorted by their long name. Note this sort will panic if
/// any [`Arg`] does not have a long name!
macro_rules! args {
    ( $arg:expr $(,)?) => {
        [$arg]
    };
    ( $( $arg:expr ),+ $(,)? ) => {
        {
            let mut args = [ $( $arg, )* ];
            args.sort_unstable_by(sort_args);
            args
        }
    };
}

const TEMPLATE: &str = indoc! {
    "{name} {version}
    {author}

    {about}

    {usage-heading} {usage}

    {all-args}"
};

const USAGE: &str = "btm [OPTIONS]";

const VERSION: &str = match option_env!("NIGHTLY_VERSION") {
    Some(nightly_version) => nightly_version,
    None => crate_version!(),
};

/// Represents the arguments that can be passed in to bottom.
#[derive(Parser, Debug)]
#[command(
    name = crate_name!(),
    version = VERSION,
    author = crate_authors!(),
    about = crate_description!(),
    disable_help_flag = true,
    disable_version_flag = true,
    color = ColorChoice::Auto,
    help_template = TEMPLATE,
    override_usage = USAGE,
)]
pub struct BottomArgs {
    #[command(flatten)]
    pub(crate) general: GeneralArgs,

    #[command(flatten)]
    pub(crate) process: ProcessArgs,

    #[command(flatten)]
    pub(crate) temperature: TemperatureArgs,

    #[command(flatten)]
    pub(crate) cpu: CpuArgs,

    #[command(flatten)]
    pub(crate) memory: MemoryArgs,

    #[command(flatten)]
    pub(crate) network: NetworkArgs,

    #[cfg(feature = "battery")]
    #[command(flatten)]
    pub(crate) battery: BatteryArgs,

    #[cfg(feature = "gpu")]
    #[command(flatten)]
    pub(crate) gpu: GpuArgs,

    #[command(flatten)]
    pub(crate) style: StyleArgs,

    #[command(flatten)]
    pub(crate) other: OtherArgs,
}

impl BottomArgs {
    /// Returns the config path if it is set.
    #[inline]
    pub fn config_path(&self) -> Option<&str> {
        self.general.config_location.as_ref().map(|p| p.as_str())
    }
}

#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "General Options")]
pub(crate) struct GeneralArgs {
    #[arg(
        long,
        help = "Temporarily shows the time scale in graphs.",
        long_help = "Automatically hides the time scale in graphs after being shown for a brief moment when zoomed \
                    in/out. If time is disabled via --hide_time then this will have no effect."
    )]
    pub(crate) autohide_time: Option<bool>,

    #[arg(
        short = 'b',
        long,
        help = "Hides graphs and uses a more basic look.",
        long_help = "Hides graphs and uses a more basic look. Design is largely inspired by htop's."
    )]
    pub(crate) basic: Option<bool>,

    #[arg(
        short = 'C',
        long,
        value_name = "PATH",
        help = "Sets the location of the config file.",
        long_help = "Sets the location of the config file. Expects a config file in the TOML format. \
                    If it doesn't exist, a default config file is created at the path. If no path is provided,
                    the default config location will be used."
    )]
    pub(crate) config_location: Option<String>,

    #[arg(
        short = 't',
        long,
        value_name = "TIME",
        help = "Default time value for graphs.",
        long_help = "The default time value for graphs. Takes a number in milliseconds or a human \
                    duration (e.g. 60s). The minimum time is 30s, and the default is 60s."
    )]
    pub(crate) default_time_value: Option<StringOrNum>,

    // TODO: Charts are broken in the manpage
    #[arg(
        long,
        requires_all = ["default_widget_type"],
        value_name = "N",
        help = "Sets the n'th selected widget type as the default. Use --help for more info.",
        long_help = indoc! {
            "Sets the n'th selected widget type to use as the default widget.
            Requires 'default_widget_type' to also be set, and defaults to 1.

            This reads from left to right, top to bottom. For example, suppose
            we have a layout that looks like:
            +-------------------+-----------------------+
            |      CPU (1)      |        CPU (2)        |
            +---------+---------+-------------+---------+
            | Process | CPU (3) | Temperature | CPU (4) |
            +---------+---------+-------------+---------+

            And we set our default widget type to 'CPU'. If we set
            '--default_widget_count 1', then it would use the CPU (1) as
            the default widget. If we set '--default_widget_count 3', it would
            use CPU (3) as the default instead."
        }
    )]
    pub(crate) default_widget_count: Option<u32>,

    #[arg(
        long,
        value_name = "WIDGET",
        value_parser = [
            "cpu",
            "mem",
            "net",
            "network",
            "proc",
            "process",
            "processes",
            "temp",
            "temperature",
            "disk",
            #[cfg(feature = "battery")]
            "batt",
            #[cfg(feature = "battery")]
            "battery",
        ],
        help = "Sets the default widget type. Use --help for more info.\n", // Newline to force the possible values to be on the next line.
        long_help = indoc!{
            "Sets which widget type to use as the default widget. For the default \
            layout, this defaults to the 'process' widget. For a custom layout, it defaults \
            to the first widget it sees.

            For example, suppose we have a layout that looks like:
            +-------------------+-----------------------+
            |      CPU (1)      |        CPU (2)        |
            +---------+---------+-------------+---------+
            | Process | CPU (3) | Temperature | CPU (4) |
            +---------+---------+-------------+---------+

            Setting '--default_widget_type Temp' will make the temperature widget selected by default."
        }
    )]
    pub(crate) default_widget_type: Option<String>,

    #[arg(
        long,
        help = "Disables mouse clicks.",
        long_help = "Disables mouse clicks from interacting with bottom."
    )]
    pub(crate) disable_click: Option<bool>,

    #[arg(
        short = 'm',
        long,
        help = "Uses a dot marker for graphs.",
        long_help = "Uses a dot marker for graphs as opposed to the default braille marker."
    )]
    pub(crate) dot_marker: Option<bool>,

    #[arg(
        short = 'e',
        long,
        help = "Expand the default widget upon starting the app.",
        long_help = "Expand the default widget upon starting the app. This flag has no effect in basic mode (--basic)."
    )]
    pub(crate) expanded: Option<bool>,

    #[arg(long, help = "Hides spacing between table headers and entries.")]
    pub(crate) hide_table_gap: Option<bool>,

    #[arg(
        long,
        help = "Hides the time scale.",
        long_help = "Completely hides the time scale from being shown."
    )]
    pub(crate) hide_time: Option<bool>,

    #[arg(
        short = 'r',
        long,
        value_name = "TIME",
        help = "Sets how often data is refreshed.",
        long_help = "Sets how often data is refreshed. Takes a number in milliseconds or a human-readable duration \
                    (e.g. 5s). The minimum is 250ms, and defaults to 1000ms. Smaller values may result in higher \
                    system usage by bottom."
    )]
    pub(crate) rate: Option<StringOrNum>,

    #[arg(
        long,
        value_name = "TIME",
        help = "The timespan of data stored.",
        long_help = "How much data is stored at once in terms of time. Takes a number in milliseconds or a \
                    human-readable duration (e.g. 20m), with a minimum of 1 minute. Note that higher values \
                    will take up more memory. Defaults to 10 minutes."
    )]
    pub(crate) retention: Option<StringOrNum>,

    #[arg(
        long,
        help = "Shows the scroll position tracker in table widgets.",
        long_help = "Shows the list scroll position tracker in the widget title for table widgets."
    )]
    pub(crate) show_table_scroll_position: Option<bool>,

    #[arg(
        short = 'd',
        long,
        value_name = "TIME",
        help = "The amount of time changed upon zooming.",
        long_help = "The amount of time changed when zooming in/out. Takes a number in milliseconds or a \
                    human-readable duration (e.g. 30s). The minimum is 1s, and defaults to 15s."
    )]
    pub(crate) time_delta: Option<StringOrNum>,
}

macro_rules! set_if_some {
    ($name:ident, $curr:expr, $new:expr) => {
        if $new.$name.is_some() {
            $curr.$name = $new.$name.clone();
        }
    };
}

impl GeneralArgs {
    pub(crate) fn merge(&mut self, other: &Self) {
        set_if_some!(autohide_time, self, other);
        set_if_some!(basic, self, other);
        set_if_some!(config_location, self, other);
        set_if_some!(default_time_value, self, other);
        set_if_some!(default_widget_count, self, other);
        set_if_some!(default_widget_type, self, other);
        set_if_some!(disable_click, self, other);
        set_if_some!(dot_marker, self, other);
        set_if_some!(expanded, self, other);
        set_if_some!(hide_time, self, other);
        set_if_some!(rate, self, other);
        set_if_some!(retention, self, other);
        set_if_some!(show_table_scroll_position, self, other);
        set_if_some!(time_delta, self, other);
    }
}

#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "Process Options")]
pub(crate) struct ProcessArgs {
    #[arg(
        short = 'S',
        long,
        help = "Enables case sensitivity by default.",
        long_help = "When searching for a process, enables case sensitivity by default."
    )]
    pub(crate) case_sensitive: Option<bool>,

    // TODO: Rename this.
    #[arg(
        short = 'u',
        long,
        help = "Sets process CPU% to be based on current CPU%.",
        long_help = "Sets process CPU% usage to be based on the current system CPU% usage rather than total CPU usage."
    )]
    pub(crate) current_usage: Option<bool>,

    // TODO: Disable this on Windows?
    #[arg(
        long,
        help = "Hides advanced process killing options.",
        long_help = "Hides advanced options to stop a process on Unix-like systems. The only \
                    option shown is 15 (TERM)."
    )]
    pub(crate) disable_advanced_kill: Option<bool>,

    #[arg(
        short = 'g',
        long,
        help = "Groups processes with the same name by default."
    )]
    pub(crate) group_processes: Option<bool>,

    #[arg(long, help = "Show processes as their commands by default.")]
    pub(crate) process_command: Option<bool>,

    #[arg(short = 'R', long, help = "Enables regex by default while searching.")]
    pub(crate) regex: Option<bool>,

    #[arg(
        short = 'T',
        long,
        help = "Defaults the process widget be in tree mode."
    )]
    pub(crate) tree: Option<bool>,

    #[arg(
        short = 'n',
        long,
        help = "Show process CPU% usage without normalizing over the number of cores.",
        long_help = "Shows all process CPU% usage without averaging over the number of CPU cores in the system."
    )]
    pub(crate) unnormalized_cpu: Option<bool>,

    #[arg(
        short = 'W',
        long,
        help = "Enables whole-word matching by default while searching."
    )]
    pub(crate) whole_word: Option<bool>,
}

impl ProcessArgs {
    pub(crate) fn merge(&mut self, other: &Self) {
        set_if_some!(case_sensitive, self, other);
        set_if_some!(current_usage, self, other);
        set_if_some!(disable_advanced_kill, self, other);
        set_if_some!(group_processes, self, other);
        set_if_some!(process_command, self, other);
        set_if_some!(regex, self, other);
        set_if_some!(tree, self, other);
        set_if_some!(unnormalized_cpu, self, other);
        set_if_some!(whole_word, self, other);
    }
}

#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "Temperature Options")]
#[group(multiple = false)]
pub(crate) struct TemperatureArgs {
    #[arg(
        short = 'c',
        long,
        group = "temperature_unit",
        help = "Use Celsius as the temperature unit. Default.",
        long_help = "Use Celsius as the temperature unit. This is the default option."
    )]
    pub(crate) celsius: Option<bool>,

    #[arg(
        short = 'f',
        long,
        group = "temperature_unit",
        help = "Use Fahrenheit as the temperature unit. Default."
    )]
    pub(crate) fahrenheit: Option<bool>,

    #[arg(
        short = 'k',
        long,
        group = "temperature_unit",
        help = "Use Kelvin as the temperature unit."
    )]
    pub(crate) kelvin: Option<bool>,
}

#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "CPU Options")]
pub(crate) struct CpuArgs {
    #[arg(long, help = "Defaults to selecting the average CPU entry.")]
    pub(crate) default_avg_cpu: Option<bool>,

    #[arg(
        short = 'a',
        long,
        help = "Hides the average CPU usage entry.",
        long = "Hides the average CPU usage entry from being shown."
    )]
    pub(crate) hide_avg_cpu: Option<bool>,

    // TODO: Maybe rename this or fix this? Should this apply to all "left legends"?
    #[arg(
        short = 'l',
        long,
        help = "Puts the CPU chart legend to the left side.",
        long_help = "Puts the CPU chart legend to the left side rather than the right side."
    )]
    pub(crate) left_legend: Option<bool>,
}

impl CpuArgs {
    pub(crate) fn merge(&mut self, other: &Self) {
        set_if_some!(default_avg_cpu, self, other);
        set_if_some!(hide_avg_cpu, self, other);
        set_if_some!(left_legend, self, other);
    }
}

#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "Memory Options")]
pub(crate) struct MemoryArgs {
    #[cfg(not(target_os = "windows"))]
    #[arg(
        long,
        help = "Enables collecting and displaying cache and buffer memory."
    )]
    pub(crate) enable_cache_memory: Option<bool>,

    #[arg(
        long,
        help = "Defaults to showing process memory usage by value.",
        long_help = "Defaults to showing process memory usage by value. Otherwise, it defaults to showing it by percentage."
    )]
    pub(crate) mem_as_value: Option<bool>,
}

impl MemoryArgs {
    pub(crate) fn merge(&mut self, other: &Self) {
        set_if_some!(enable_cache_memory, self, other);
        set_if_some!(mem_as_value, self, other);
    }
}

#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "Network Options")]
pub(crate) struct NetworkArgs {
    #[arg(
        long,
        help = "Displays the network widget using bytes.",
        long_help = "Displays the network widget using bytes. Defaults to bits."
    )]
    pub(crate) network_use_bytes: Option<bool>,

    #[arg(
        long,
        help = "Displays the network widget with binary prefixes.",
        long_help = "Displays the network widget with binary prefixes (e.g. kibibits, mebibits) rather than a decimal \
                    prefixes (e.g. kilobits, megabits). Defaults to decimal prefixes."
    )]
    pub(crate) network_use_binary_prefix: Option<bool>,

    #[arg(
        long,
        help = "Displays the network widget with a log scale.",
        long_help = "Displays the network widget with a log scale. Defaults to a non-log scale."
    )]
    pub(crate) network_use_log: Option<bool>,

    #[arg(
        long,
        help = "(DEPRECATED) Uses a separate network legend.",
        long_help = "(DEPRECATED) Uses separate network widget legend. This display is not tested and may be broken."
    )]
    pub(crate) use_old_network_legend: Option<bool>,
}

impl NetworkArgs {
    pub(crate) fn merge(&mut self, other: &Self) {
        set_if_some!(network_use_bytes, self, other);
        set_if_some!(network_use_binary_prefix, self, other);
        set_if_some!(network_use_log, self, other);
        set_if_some!(use_old_network_legend, self, other);
    }
}

#[cfg(feature = "battery")]
#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "Battery Options")]
pub(crate) struct BatteryArgs {
    #[arg(
        long,
        help = "Shows the battery widget in default/basic mode.",
        long_help = "Shows the battery widget in default or basic mode, if there is as battery available. This \
                    has no effect on custom layouts; if the battery widget is desired for a custom layout, explicitly \
                    specify it."
    )]
    pub(crate) battery: Option<bool>,
}

#[cfg(feature = "battery")]

impl BatteryArgs {
    pub(crate) fn merge(&mut self, other: &Self) {
        set_if_some!(battery, self, other);
    }
}

#[cfg(feature = "gpu")]
#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "GPU Options")]
pub(crate) struct GpuArgs {
    #[arg(long, help = "Enables collecting and displaying GPU usage.")]
    pub(crate) enable_gpu: Option<bool>,
}

#[cfg(feature = "gpu")]
impl GpuArgs {
    pub(crate) fn merge(&mut self, other: &Self) {
        set_if_some!(enable_gpu, self, other);
    }
}

#[derive(Args, Clone, Debug, Default, Deserialize)]
#[command(next_help_heading = "Style Options")]
pub(crate) struct StyleArgs {
    #[arg(
        long,
        value_name="SCHEME",
        value_parser=[
            "default",
            "default-light",
            "gruvbox",
            "gruvbox-light",
            "nord",
            "nord-light",

        ],
        hide_possible_values=true,
        help = "Use a color scheme, use --help for info on the colors.\n
                [possible values: default, default-light, gruvbox, gruvbox-light, nord, nord-light]",
        long_help=indoc! {
            "Use a pre-defined color scheme. Currently supported values are:
            - default
            - default-light (default but adjusted for lighter backgrounds)
            - gruvbox       (a bright theme with 'retro groove' colors)
            - gruvbox-light (gruvbox but adjusted for lighter backgrounds)
            - nord          (an arctic, north-bluish color palette)
            - nord-light    (nord but adjusted for lighter backgrounds)"
        }
    )]
    pub(crate) color: Option<String>,
}

impl StyleArgs {
    pub(crate) fn merge(&mut self, other: &Self) {
        set_if_some!(color, self, other);
    }
}

#[derive(Args, Clone, Debug)]
#[command(next_help_heading = "Other Options")]
pub(crate) struct OtherArgs {
    #[arg(short='h', long, action=ArgAction::Help, help="Prints help info (for more details use `--help`.")]
    help: (),

    #[arg(short='v', long, action=ArgAction::Version, help="Prints version information.")]
    version: (),
}

/// Returns a [`BottomArgs`].
pub fn get_args() -> BottomArgs {
    BottomArgs::parse()
}

/// Returns an [`Command`] based off of [`BottomArgs`].
fn build_cmd() -> Command {
    BottomArgs::command()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn verify_cli() {
        build_cmd().debug_assert();
    }

    #[test]
    fn no_default_help_heading() {
        let mut cmd = build_cmd();
        let help_str = cmd.render_help();

        assert!(
            !help_str.to_string().contains("\nOptions:\n"),
            "the default 'Options' heading should not exist; if it does then an argument is \
            missing a help heading."
        );
    }
}
