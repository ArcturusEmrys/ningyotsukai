use std::borrow::Cow;

pub trait StrExt: ToOwned {
    /// Replace nulls with \0 for round-trippable display.
    ///
    /// Several Inochi files I have put trailing nulls everywhere.
    /// GTK chokes on them.
    /// This fixes that.
    fn escape_nulls<'a>(&'a self) -> Cow<'a, Self>;
}

impl StrExt for str {
    fn escape_nulls<'a>(&'a self) -> Cow<'a, Self> {
        if !self.contains("\0") && !self.contains("\\") {
            return self.into();
        }

        self.replace("\\", "\\\\").replace("\0", "\\0").into()
    }
}
