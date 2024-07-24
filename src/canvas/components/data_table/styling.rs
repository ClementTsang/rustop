use tui::style::Style;

use crate::options::config::style::ColourPalette;

#[derive(Default)]
pub struct DataTableStyling {
    pub header_style: Style,
    pub border_style: Style,
    pub highlighted_border_style: Style,
    pub text_style: Style,
    pub highlighted_text_style: Style,
    pub title_style: Style,
}

impl DataTableStyling {
    pub fn from_colours(colours: &ColourPalette) -> Self {
        Self {
            header_style: colours.table_header_style,
            border_style: colours.border_style,
            highlighted_border_style: colours.highlighted_border_style,
            text_style: colours.text_style,
            highlighted_text_style: colours.selected_text_style,
            title_style: colours.widget_title_style,
        }
    }
}
