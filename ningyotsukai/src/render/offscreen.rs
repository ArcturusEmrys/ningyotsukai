use crate::document::{Document, WeakDocument};

pub struct OffscreenRender {
    /// The document we want to render.
    ///
    /// This is a weak reference to allow us to know if we should discard
    /// associated resources when the document is closed.
    document: WeakDocument,
}

impl OffscreenRender {
    pub fn new(document: Document) -> Self {
        OffscreenRender {
            document: document.downgrade(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.document.upgrade().is_some()
    }

    pub fn is_for_document(&self, other_document: &Document) -> bool {
        if let Some(my_doc) = self.document.upgrade() {
            &my_doc == other_document
        } else {
            false
        }
    }
}
