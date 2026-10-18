use bevy::prelude::*;

pub const BUTTON_COLOR: Color = Color::srgb(0.2, 0.2, 0.2);
pub const BUTTON_HOVERED_COLOR: Color = Color::srgb(0.3, 0.3, 0.3);
pub const BUTTON_PRESSED_COLOR: Color = Color::srgb(0.4, 0.4, 0.4);
pub const BUTTON_TEXT_COLOR: Color = Color::WHITE;
pub const BUTTON_FONT_SIZE: f32 = 28.;
pub const BUTTON_BORDER_RADIUS: Val = Val::Px(10.);

/// Registers the systems that keep standard-styled elements in sync with their interaction state.
pub struct StylePlugin;

impl Plugin for StylePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_button_colors);
    }
}

pub fn button_node() -> Node {
    Node {
        width: Val::Px(200.),
        height: Val::Px(50.),
        justify_content: JustifyContent::Center,
        align_items: AlignItems::Center,
        border_radius: BorderRadius::all(BUTTON_BORDER_RADIUS),
        ..default()
    }
}

pub fn button_text(label: impl Into<String>) -> impl Bundle {
    (
        Text::new(label),
        TextFont {
            font_size: BUTTON_FONT_SIZE.into(),
            ..default()
        },
        TextColor(BUTTON_TEXT_COLOR),
    )
}

/// A standard button with a text label; add a marker component next to it to tell buttons apart.
pub fn button(label: impl Into<String>) -> impl Bundle {
    (
        Button,
        button_node(),
        BackgroundColor(BUTTON_COLOR),
        children![button_text(label)],
    )
}

fn update_button_colors(
    mut q: Query<(&Interaction, &mut BackgroundColor), (Changed<Interaction>, With<Button>)>,
) {
    for (interaction, mut background) in &mut q {
        background.0 = match interaction {
            Interaction::Pressed => BUTTON_PRESSED_COLOR,
            Interaction::Hovered => BUTTON_HOVERED_COLOR,
            Interaction::None => BUTTON_COLOR,
        };
    }
}
