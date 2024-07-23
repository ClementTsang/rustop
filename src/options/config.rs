pub mod cpu;
pub mod disk;
pub mod flags;
mod ignore_list;
pub mod layout;
pub mod network;
pub mod process;
pub mod temperature;

use disk::DiskConfig;
use flags::FlagConfig;
use network::NetworkConfig;
use serde::{Deserialize, Serialize};
use temperature::TempConfig;

pub use self::ignore_list::IgnoreList;
use self::{cpu::CpuConfig, layout::Row, process::ProcessesConfig};
use super::ColoursConfig;

#[derive(Clone, Debug, Default, Deserialize)]
#[cfg_attr(
    feature = "generate_schema",
    derive(schemars::JsonSchema),
    schemars(title = "Schema for bottom's configs (nightly)")
)]
pub struct ConfigV1 {
    pub(crate) flags: Option<FlagConfig>,
    pub(crate) colors: Option<ColoursConfig>,
    pub(crate) row: Option<Vec<Row>>,
    pub(crate) processes: Option<ProcessesConfig>,
    pub(crate) disk: Option<DiskConfig>,
    pub(crate) temperature: Option<TempConfig>,
    pub(crate) network: Option<NetworkConfig>,
    pub(crate) cpu: Option<CpuConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "generate_schema", derive(schemars::JsonSchema))]
pub(crate) enum StringOrNum {
    String(String),
    Num(u64),
}

impl From<String> for StringOrNum {
    fn from(value: String) -> Self {
        StringOrNum::String(value)
    }
}

impl From<u64> for StringOrNum {
    fn from(value: u64) -> Self {
        StringOrNum::Num(value)
    }
}
