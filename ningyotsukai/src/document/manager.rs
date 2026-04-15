use std::cell::RefCell;
use std::rc::Rc;
use std::thread::spawn;

pub struct DocumentManager(Rc<RefCell<DocumentManagerInner>>);
struct DocumentManagerInner {}

impl DocumentManager {
    pub fn new() -> Self {
        DocumentManager(Rc::new(RefCell::new(DocumentManagerInner {})))
    }
}
