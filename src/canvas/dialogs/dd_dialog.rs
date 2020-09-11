use tui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    terminal::Frame,
    widgets::{Block, Borders, Paragraph, Text},
};

use crate::{app::App, canvas::Painter};

const DD_BASE: &str = " Confirm Kill Process ── Esc to close ";
const DD_ERROR_BASE: &str = " Error ── Esc to close ";

pub trait KillDialog {
    fn get_dd_spans(&self, app_state: &App) -> Option<Vec<Text<'_>>>;

    fn draw_dd_dialog<B: Backend>(
        &self, f: &mut Frame<'_, B>, dd_text: Option<Vec<Text<'_>>>, app_state: &mut App,
        draw_loc: Rect,
    ) -> bool;
}

impl KillDialog for Painter {
    fn get_dd_spans(&self, app_state: &App) -> Option<Vec<Text<'_>>> {
        if let Some(dd_err) = &app_state.dd_err {
            return Some(vec![
                Text::raw("\n"),
                Text::raw(format!("Failed to kill process.\n{}\n", dd_err)),
                Text::raw("Please press ENTER or ESC to close this dialog."),
            ]);
        } else if let Some(to_kill_processes) = app_state.get_to_delete_processes() {
            if let Some(first_pid) = to_kill_processes.1.first() {
                return Some(vec![
                    Text::raw("\n"),
                    if app_state.is_grouped(app_state.current_widget.widget_id) {
                        if to_kill_processes.1.len() != 1 {
                            Text::raw(format!(
                                "Kill {} processes with the name \"{}\"?",
                                to_kill_processes.1.len(),
                                to_kill_processes.0
                            ))
                        } else {
                            Text::raw(format!(
                                "Kill 1 process with the name \"{}\"?",
                                to_kill_processes.0
                            ))
                        }
                    } else {
                        Text::raw(format!(
                            "Kill process \"{}\" with PID {}?",
                            to_kill_processes.0, first_pid
                        ))
                    },
                ]);
            }
        }

        None
    }

    fn draw_dd_dialog<B: Backend>(
        &self, f: &mut Frame<'_, B>, dd_text: Option<Vec<Text<'_>>>, app_state: &mut App,
        draw_loc: Rect,
    ) -> bool {
        if let Some(dd_text) = dd_text {
            // let dd_title = if app_state.dd_err.is_some() {
            //     Text::styled(
            //         format!(
            //             " Error ─{}─ Esc to close ",
            //             "─".repeat(
            //                 usize::from(draw_loc.width)
            //                     .saturating_sub(DD_ERROR_BASE.chars().count() + 2)
            //             )
            //         ),
            //         self.colours.border_style,
            //     )
            // } else {
            //     Text::styled(
            //         format!(
            //             " Confirm Kill Process ─{}─ Esc to close ",
            //             "─".repeat(
            //                 usize::from(draw_loc.width).saturating_sub(DD_BASE.chars().count() + 2)
            //             )
            //         ),
            //         self.colours.border_style,
            //     )
            // };

            let dd_title = if app_state.dd_err.is_some() {
                format!(
                    " Error ─{}─ Esc to close ",
                    "─".repeat(
                        usize::from(draw_loc.width)
                            .saturating_sub(DD_ERROR_BASE.chars().count() + 2)
                    )
                )
            } else {
                format!(
                    " Confirm Kill Process ─{}─ Esc to close ",
                    "─".repeat(
                        usize::from(draw_loc.width).saturating_sub(DD_BASE.chars().count() + 2)
                    )
                )
            };

            f.render_widget(
                Paragraph::new(dd_text.iter())
                    .block(
                        Block::default()
                            .title(&dd_title)
                            .title_style(self.colours.border_style)
                            .style(self.colours.border_style)
                            .borders(Borders::ALL)
                            .border_style(self.colours.border_style),
                    )
                    .style(self.colours.text_style)
                    .alignment(Alignment::Center)
                    .wrap(true),
                draw_loc,
            );

            // Now draw buttons if needed...
            let split_draw_loc = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    if app_state.dd_err.is_some() {
                        vec![Constraint::Percentage(100)]
                    } else {
                        vec![Constraint::Min(0), Constraint::Length(3)]
                    }
                    .as_ref(),
                )
                .split(draw_loc);

            // This being true implies that dd_err is none.
            if let Some(button_draw_loc) = split_draw_loc.get(1) {
                let (yes_button, no_button) = if app_state.delete_dialog_state.is_on_yes {
                    (
                        Text::styled("Yes", self.colours.currently_selected_text_style),
                        Text::raw("No"),
                    )
                } else {
                    (
                        Text::raw("Yes"),
                        Text::styled("No", self.colours.currently_selected_text_style),
                    )
                };

                let button_layout = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints(
                        [
                            Constraint::Percentage(35),
                            Constraint::Percentage(30),
                            Constraint::Percentage(35),
                        ]
                        .as_ref(),
                    )
                    .split(*button_draw_loc);

                f.render_widget(
                    Paragraph::new([yes_button].iter())
                        .block(Block::default())
                        .alignment(Alignment::Right),
                    button_layout[0],
                );
                f.render_widget(
                    Paragraph::new([no_button].iter())
                        .block(Block::default())
                        .alignment(Alignment::Left),
                    button_layout[2],
                );

                if app_state.should_get_widget_bounds() {
                    app_state.delete_dialog_state.yes_tlc =
                        Some((button_layout[0].x, button_layout[0].y));
                    app_state.delete_dialog_state.yes_brc = Some((
                        button_layout[0].x + button_layout[0].width,
                        button_layout[0].y + button_layout[0].height,
                    ));

                    app_state.delete_dialog_state.no_tlc =
                        Some((button_layout[2].x, button_layout[2].y));
                    app_state.delete_dialog_state.no_brc = Some((
                        button_layout[2].x + button_layout[2].width,
                        button_layout[2].y + button_layout[2].height,
                    ));
                }
            }

            if app_state.dd_err.is_some() {
                return app_state.delete_dialog_state.is_showing_dd;
            } else {
                return true;
            }
        }

        // Currently we just return "false" if things go wrong finding
        // the process or a first PID (if an error arises it should be caught).
        // I don't really like this, and I find it ugly, but it works for now.
        false
    }
}
