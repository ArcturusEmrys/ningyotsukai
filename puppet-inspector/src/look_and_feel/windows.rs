use gtk4;

use windows::Foundation::TypedEventHandler;
use windows::UI::Color;
use windows::UI::ViewManagement::{UIColorType, UISettings};

use std::cell::RefCell;

fn wcag_value_to_float(v: u8) -> f32 {
    let vf = v as f32 / 255.0;
    if vf < 0.03928 {
        vf / 12.92
    } else {
        ((vf + 0.055) / 1.055).powf(2.4)
    }
}

trait ColorExt: Sized + Copy {
    fn into_css_hex(self) -> String;
    fn wcag_relative_luma(self) -> f32;

    fn contrast_ratio(self, counter: Self) -> f32 {
        let self_luma = self.wcag_relative_luma();
        let counter_luma = counter.wcag_relative_luma();

        if self_luma > counter_luma {
            (self_luma + 0.05) / (counter_luma + 0.05)
        } else {
            (counter_luma + 0.05) / (self_luma + 0.05)
        }
    }

    fn contrast(self, counter1: Self, counter2: Self) -> Self {
        let ratio1 = self.contrast_ratio(counter1);
        let ratio2 = self.contrast_ratio(counter2);

        if ratio1 > ratio2 { counter1 } else { counter2 }
    }
}

impl ColorExt for Color {
    fn into_css_hex(self) -> String {
        format!("#{:02X}{:02X}{:02X}{:02X}", self.R, self.G, self.B, self.A)
    }

    fn wcag_relative_luma(self) -> f32 {
        let r = wcag_value_to_float(self.R);
        let g = wcag_value_to_float(self.G);
        let b = wcag_value_to_float(self.B);

        0.2126 * r + 0.7152 * g + 0.0722 * b
    }
}

pub struct LAFProvider {
    ui_settings: UISettings,
    color_css: gtk4::CssProvider,
    laf_css: gtk4::CssProvider,
}

thread_local! {
    static LAF_SINGLETON: RefCell<Option<LAFProvider>> = RefCell::new(None);
}

impl LAFProvider {
    pub fn init() {
        if LAF_SINGLETON.with(|laf| laf.borrow().is_some()) {
            return;
        }

        let laf_css = gtk4::CssProvider::new();
        laf_css.load_from_resource("/live/arcturus/puppet-inspector/windows-look-and-feel.css");

        let color_css = gtk4::CssProvider::new();

        let display = gdk4::Display::default().expect("display");
        gtk4::style_context_add_provider_for_display(
            &display,
            &color_css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
        gtk4::style_context_add_provider_for_display(
            &display,
            &laf_css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        LAF_SINGLETON.with(|laf| {
            *laf.borrow_mut() = Some(Self {
                ui_settings: UISettings::new().expect("working windows environment"),
                color_css,
                laf_css,
            });
        });

        Self::with_singleton(|laf| {
            laf.update_color_css();

            laf.ui_settings
                .ColorValuesChanged(Some(&TypedEventHandler::new(|_, _| {
                    glib::idle_add_once(|| {
                        LAFProvider::with_singleton(|laf| laf.update_color_css());
                    });
                    Ok(())
                })))
                .expect("validly registered handler");
        });
    }

    fn with_singleton<T, F: FnOnce(&LAFProvider) -> T>(the_fn: F) -> T {
        Self::init();

        LAF_SINGLETON.with(|laf| {
            the_fn(
                laf.borrow()
                    .as_ref()
                    .expect("globals must stay initialized"),
            )
        })
    }

    fn update_color_css(&self) {
        let bg_color = self
            .ui_settings
            .GetColorValue(UIColorType::Background)
            .expect("bg color");
        let bg_color_hex = bg_color.into_css_hex();
        let fg_color = self
            .ui_settings
            .GetColorValue(UIColorType::Foreground)
            .expect("fg color")
            .into_css_hex();
        let accent_color = self
            .ui_settings
            .GetColorValue(UIColorType::Accent)
            .expect("accent color");

        let white = Color {
            A: 255,
            R: 255,
            G: 255,
            B: 255,
        };
        let black = Color {
            A: 255,
            R: 0,
            G: 0,
            B: 0,
        };

        let contrast_color = accent_color.contrast(white, black).into_css_hex();
        let accent_color = accent_color.into_css_hex();

        // GTK on Windows does NOT automatically detect dark/light.
        // Which is very bad as it ships with Adwaita look and feel which supports it.
        // Also, for some reason our own CSS doesn't pull color scheme data from settings.
        let color_scheme = if bg_color.contrast(white, black) == white {
            gtk4::InterfaceColorScheme::Dark
        } else {
            gtk4::InterfaceColorScheme::Light
        };
        gtk4::Settings::default()
            .expect("wot no default")
            .set_gtk_interface_color_scheme(color_scheme);
        self.laf_css.set_prefers_color_scheme(color_scheme);
        self.color_css.set_prefers_color_scheme(color_scheme);

        let accent_light1_color = self
            .ui_settings
            .GetColorValue(UIColorType::AccentLight1)
            .expect("accent color")
            .into_css_hex();
        let accent_light2_color = self
            .ui_settings
            .GetColorValue(UIColorType::AccentLight2)
            .expect("accent color")
            .into_css_hex();
        let accent_light3_color = self
            .ui_settings
            .GetColorValue(UIColorType::AccentLight3)
            .expect("accent color")
            .into_css_hex();
        let accent_dark1_color = self
            .ui_settings
            .GetColorValue(UIColorType::AccentDark1)
            .expect("accent color")
            .into_css_hex();
        let accent_dark2_color = self
            .ui_settings
            .GetColorValue(UIColorType::AccentDark2)
            .expect("accent color")
            .into_css_hex();
        let accent_dark3_color = self
            .ui_settings
            .GetColorValue(UIColorType::AccentDark3)
            .expect("accent color")
            .into_css_hex();

        //I'm surprised GTK doesn't have a convenience method to define CSS variables.
        let css = format!(
            ":root {{
            --Windows-background: {bg_color_hex};
            --Windows-foreground: {fg_color};
            --Windows-accent: {accent_color};
            --contrasts_with_Windows_accent: {contrast_color};
            --Windows-accent--light1: {accent_light1_color};
            --Windows-accent--light2: {accent_light2_color};
            --Windows-accent--light3: {accent_light3_color};
            --Windows-accent--dark1: {accent_dark1_color};
            --Windows-accent--dark2: {accent_dark2_color};
            --Windows-accent--dark3: {accent_dark3_color};
        }}"
        );

        #[allow(deprecated)] //Ubuntu requires 4.10 support
        self.color_css.load_from_data(&css);
    }
}

pub fn init() {
    LAFProvider::init();
}
