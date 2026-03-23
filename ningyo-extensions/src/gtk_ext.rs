use gtk4;
use gtk4::prelude::*;

/// Iterator that walks the widget tree from a given element.
struct WalkIter {
    stack: Vec<gtk4::Widget>,
}

impl Iterator for WalkIter {
    type Item = gtk4::Widget;

    fn next(&mut self) -> Option<Self::Item> {
        let current = (*self.stack.last()?).clone();

        if let Some(child) = current.first_child() {
            self.stack.push(child);
        } else {
            while self.stack.len() > 0 {
                if let Some(sibling) = self.stack.pop().unwrap().next_sibling() {
                    self.stack.push(sibling);
                    break;
                }
            }
        }

        Some(current)
    }
}

impl WalkIter {
    pub fn new(start: gtk4::Widget) -> Self {
        Self { stack: vec![start] }
    }
}

pub trait WidgetExt2: WidgetExt {
    /// Get the widget's current window, or None if the widget is not yet
    /// realized.
    fn window(&self) -> Option<gtk4::Window> {
        // GTK3 had a get_window, GTK4 removed it.
        // Dunno why, but there's a forum thread where someone
        // basically says you shouldn't need to get the current
        // window because you don't need to touch GDK as often.
        // Guess he didn't read GTK's own alert API.
        self.closest()
    }

    /// Get the closest parent of a given type.
    ///
    /// Analogous to HTML DOM .closest(), but takes a generic param instead of
    /// a CSS selector.
    fn closest<T>(&self) -> Option<T>
    where
        T: IsA<gtk4::Widget>,
    {
        let mut maybe_target_type = self.parent();
        let mut target_type = None;
        while maybe_target_type.is_some() {
            if let Some(atarget_type) = maybe_target_type.as_ref().unwrap().downcast_ref::<T>() {
                target_type = Some(atarget_type.clone());
                break;
            }

            maybe_target_type = maybe_target_type.unwrap().parent();
        }

        target_type
    }

    /// Walk all children of this widget in depth-first order.
    fn walk(&self) -> impl Iterator<Item = gtk4::Widget> {
        WalkIter::new(self.clone().upcast::<gtk4::Widget>())
    }

    /// Find all child widgets in the widget hierarchy of a given type.
    fn find_all<T>(&self) -> impl Iterator<Item = T>
    where
        T: IsA<gtk4::Widget>,
    {
        self.walk().filter_map(|v| v.downcast::<T>().ok())
    }
}

impl<O: IsA<gtk4::Widget>> WidgetExt2 for O {}
