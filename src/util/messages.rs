use serenity::all::{ButtonStyle, CreateActionRow, CreateButton};

use crate::util::component_ids::{LIST_NEXT_BUTTON, LIST_PREV_BUTTON};

pub fn create_button(id: &str, label: &str, enabled: bool) -> CreateButton {
    let button = CreateButton::new(id)
        .label(label)
        .style(ButtonStyle::Secondary)
        .disabled(!enabled);

    return button;
}

pub fn create_navigation_buttons(prev: bool, next: bool) -> Vec<CreateActionRow> {
    vec![CreateActionRow::Buttons(vec![
        create_button(LIST_PREV_BUTTON, "Previous", prev),
        create_button(LIST_NEXT_BUTTON, "Next", next)
    ])]
}