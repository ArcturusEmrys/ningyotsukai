use gio;
use gio::prelude::*;
use std::io::{Error, ErrorKind, Read};

/// Adaptor for using GIO files as regular Rust streams.
///
/// I'm genuinely surprised gtk-rs does not ship with these.
pub struct FileIn<T>(T);

impl<T> From<T> for FileIn<T>
where
    T: InputStreamExt,
{
    fn from(fis: T) -> Self {
        Self(fis)
    }
}

impl<T> Read for FileIn<T>
where
    T: InputStreamExt,
{
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        self.0
            .read(buf, None::<&gio::Cancellable>)
            .map_err(|gerror| Error::new(ErrorKind::Other, gerror))
    }
}
