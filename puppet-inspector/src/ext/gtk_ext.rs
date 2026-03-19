use gtk4;
use gtk4::prelude::*;

pub trait WidgetExt2: WidgetExt {
    /// Get the widget's current window, or None if the widget is not yet
    /// realized.
    fn window(&self) -> Option<gtk4::Window> {
        // GTK3 had a get_window, GTK4 removed it.
        // Dunno why, but there's a forum thread where someone
        // basically says you shouldn't need to get the current
        // window because you don't need to touch GDK as often.
        // Guess he didn't read GTK's own alert API.
        let mut maybe_window = self.parent();
        let mut window = None;
        while maybe_window.is_some() {
            if let Some(awindow) = maybe_window
                .as_ref()
                .unwrap()
                .downcast_ref::<gtk4::Window>()
            {
                window = Some(awindow.clone());
                break;
            }

            maybe_window = maybe_window.unwrap().parent();
        }

        window
    }
}

impl<O: IsA<gtk4::Widget>> WidgetExt2 for O {}
