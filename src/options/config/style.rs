//! Config options around styling.

mod battery;
mod cpu;
mod graph;
mod memory;
mod network;
mod table;
mod themes;
mod utils;
mod widget;

use std::borrow::Cow;

use battery::BatteryStyle;
use cpu::CpuStyle;
use graph::GraphStyle;
use memory::MemoryStyle;
use network::NetworkStyle;
use serde::{Deserialize, Serialize};
use table::TableStyle;
use tui::style::Style;
use utils::{opt, set_colour, set_colour_list, set_style};
use widget::WidgetStyle;

use crate::options::{args::BottomArgs, OptionError, OptionResult};

use super::Config;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "generate_schema", derive(schemars::JsonSchema))]
pub(crate) struct ColorStr(Cow<'static, str>);

/// A style for text.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "generate_schema", derive(schemars::JsonSchema))]
pub(crate) struct TextStyleConfig {
    /// A built-in ANSI colour, RGB hex, or RGB colour code.
    #[serde(alias = "colour")]
    pub(crate) color: Option<ColorStr>,

    /// A built-in ANSI colour, RGB hex, or RGB colour code.
    #[serde(alias = "bg_colour")]
    pub(crate) bg_color: Option<ColorStr>,

    /// Whether to make this text bolded or not. If not set,
    /// will default to built-in defaults.
    pub(crate) bold: Option<bool>,
}

/// Style-related configs.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "generate_schema", derive(schemars::JsonSchema))]
pub(crate) struct StyleConfig {
    /// A built-in theme.
    ///
    /// If this is and a custom colour are both set, in the config file,
    /// the custom colour scheme will be prioritized first. If a theme
    /// is set in the command-line args, however, it will always be
    /// prioritized first.
    pub(crate) theme: Option<Cow<'static, str>>,

    /// Styling for the CPU widget.
    pub(crate) cpu: Option<CpuStyle>,

    /// Styling for the memory widget.
    pub(crate) memory: Option<MemoryStyle>,

    /// Styling for the network widget.
    pub(crate) network: Option<NetworkStyle>,

    /// Styling for the battery widget.
    pub(crate) battery: Option<BatteryStyle>,

    /// Styling for table widgets.
    pub(crate) tables: Option<TableStyle>,

    /// Styling for graph widgets.
    pub(crate) graphs: Option<GraphStyle>,

    /// Styling for general widgets.
    pub(crate) widgets: Option<WidgetStyle>,
}

/// The actual internal representation of the configured colours,
/// as a "palette".
#[derive(Debug)]
pub struct ColourPalette {
    pub selected_text_style: Style,
    pub table_header_style: Style,
    pub ram_style: Style,
    #[cfg(not(target_os = "windows"))]
    pub cache_style: Style,
    pub swap_style: Style,
    pub arc_style: Style,
    pub gpu_colours: Vec<Style>,
    pub rx_style: Style,
    pub tx_style: Style,
    pub total_rx_style: Style,
    pub total_tx_style: Style,
    pub all_cpu_colour: Style,
    pub avg_cpu_colour: Style,
    pub cpu_colour_styles: Vec<Style>,
    pub border_style: Style,
    pub highlighted_border_style: Style,
    pub text_style: Style,
    pub widget_title_style: Style,
    pub graph_style: Style,
    pub graph_legend_style: Style,
    pub high_battery: Style,
    pub medium_battery: Style,
    pub low_battery: Style,
    pub invalid_query_style: Style,
    pub disabled_text_style: Style,
}

impl Default for ColourPalette {
    fn default() -> Self {
        Self::default_palette()
    }
}

impl ColourPalette {
    pub fn new(args: &BottomArgs, config: &Config) -> anyhow::Result<Self> {
        let mut palette = match &args.style.theme {
            Some(theme) => Self::from_theme(theme)?,
            None => match config.styles.as_ref().and_then(|s| s.theme.as_ref()) {
                Some(theme) => Self::from_theme(theme)?,
                None => Self::default(),
            },
        };

        // Apply theme from config on top.
        if let Some(style) = &config.styles {
            palette.set_colours_from_palette(style)?;
        }

        Ok(palette)
    }

    fn from_theme(theme: &str) -> anyhow::Result<Self> {
        let lower_case = theme.to_lowercase();
        match lower_case.as_str() {
            "default" => Ok(Self::default_palette()),
            "default-light" => Ok(Self::default_light_mode()),
            "gruvbox" => Ok(Self::gruvbox_palette()),
            "gruvbox-light" => Ok(Self::gruvbox_light_palette()),
            "nord" => Ok(Self::nord_palette()),
            "nord-light" => Ok(Self::nord_light_palette()),
            _ => Err(
                OptionError::other(format!("'{theme}' is an invalid built-in color scheme."))
                    .into(),
            ),
        }
    }

    fn set_colours_from_palette(&mut self, config: &StyleConfig) -> OptionResult<()> {
        // CPU
        set_colour!(self.avg_cpu_colour, config.cpu, avg_entry_color);
        set_colour!(self.all_cpu_colour, config.cpu, all_entry_color);
        set_colour_list!(self.cpu_colour_styles, config.cpu, cpu_core_colors);

        // Memory
        set_colour!(self.ram_style, config.memory, ram);
        set_colour!(self.swap_style, config.memory, swap);

        #[cfg(not(target_os = "windows"))]
        set_colour!(self.cache_style, config.memory, cache);

        #[cfg(feature = "zfs")]
        set_colour!(self.arc_style, config.memory, arc);

        #[cfg(feature = "gpu")]
        set_colour_list!(self.gpu_colours, config.memory, gpus);

        // Network
        set_colour!(self.rx_style, config.network, rx);
        set_colour!(self.tx_style, config.network, tx);
        set_colour!(self.total_rx_style, config.network, rx_total);
        set_colour!(self.total_tx_style, config.network, tx_total);

        // Battery
        set_colour!(self.high_battery, config.battery, high_battery);
        set_colour!(self.medium_battery, config.battery, medium_battery);
        set_colour!(self.low_battery, config.battery, low_battery);

        // Tables
        set_style!(self.table_header_style, config.tables, headers);

        // Widget graphs
        set_colour!(self.graph_style, config.graphs, graph_color);
        set_style!(self.graph_legend_style, config.graphs, legend_text);

        // General widget text.
        set_style!(self.widget_title_style, config.widgets, widget_title);
        set_style!(self.text_style, config.widgets, text);
        set_style!(self.selected_text_style, config.widgets, selected_text);
        set_style!(self.disabled_text_style, config.widgets, disabled_text);

        // Widget borders
        set_colour!(self.border_style, config.widgets, border);
        set_colour!(
            self.highlighted_border_style,
            config.widgets,
            highlighted_border_color
        );

        Ok(())
    }
}

#[cfg(test)]
mod test {

    use tui::style::{Color, Style};

    use super::ColourPalette;
    use crate::options::config::style::utils::str_to_colour;

    #[test]
    fn default_selected_colour_works() {
        let mut colours = ColourPalette::default();
        println!("colours: {colours:?}");
        let original_selected_text_colour = ColourPalette::default_palette()
            .selected_text_style
            .fg
            .unwrap();
        let original_selected_bg_colour = ColourPalette::default_palette()
            .selected_text_style
            .bg
            .unwrap();

        assert_eq!(
            colours.selected_text_style,
            Style::default()
                .fg(original_selected_text_colour)
                .bg(original_selected_bg_colour),
        );

        colours.selected_text_style = colours
            .selected_text_style
            .fg(str_to_colour("magenta").unwrap())
            .bg(str_to_colour("red").unwrap());

        assert_eq!(
            colours.selected_text_style,
            Style::default().fg(Color::Magenta).bg(Color::Red),
        );
    }

    #[test]
    fn built_in_colour_schemes_work() {
        ColourPalette::from_theme("default").unwrap();
        ColourPalette::from_theme("default-light").unwrap();
        ColourPalette::from_theme("gruvbox").unwrap();
        ColourPalette::from_theme("gruvbox-light").unwrap();
        ColourPalette::from_theme("nord").unwrap();
        ColourPalette::from_theme("nord-light").unwrap();
    }
}
