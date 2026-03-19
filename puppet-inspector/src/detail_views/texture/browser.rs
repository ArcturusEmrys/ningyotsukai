use glib;
use glib::subclass::InitializingObject;
use glib::subclass::prelude::*;

use gdk4;

use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use crate::document::Document;
use crate::navigation::{NavigationItem, Path};

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/detail_views/texture/browser.ui")]
pub struct TextureBrowserImp {
    state: RefCell<Option<Arc<Mutex<Document>>>>,

    #[template_child]
    texture_view: TemplateChild<gtk4::GridView>,
    #[template_child]
    texture_selection: TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    texture_factory: TemplateChild<gtk4::SignalListItemFactory>,
}

#[glib::object_subclass]
impl ObjectSubclass for TextureBrowserImp {
    const NAME: &'static str = "PIPuppetTextureBrowser";
    type Type = TextureBrowser;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for TextureBrowserImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for TextureBrowserImp {}

impl BoxImpl for TextureBrowserImp {}

glib::wrapper! {
    pub struct TextureBrowser(ObjectSubclass<TextureBrowserImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl TextureBrowser {
    pub fn new(document: Arc<Mutex<Document>>) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().state.borrow_mut() = Some(document);
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let document_arc = self.imp().state.borrow().as_ref().unwrap().clone();

        let bind_document = document_arc.clone();
        self.imp().texture_factory.connect_bind(move |_, row| {
            let list_item = row.downcast_ref::<gtk4::ListItem>().unwrap();
            let nav_path = list_item
                .item()
                .unwrap()
                .downcast_ref::<NavigationItem>()
                .unwrap()
                .as_path();
            let Path::ModelTexture(texture_id) = nav_path else {
                panic!("I should never get a non-model path");
            };

            let mut document = bind_document.lock().unwrap();
            let textures = document.textures();

            if let Some(my_texture) = textures.get(texture_id as usize) {
                let tex = gdk4::MemoryTexture::new(
                    my_texture.width() as i32,
                    my_texture.height() as i32,
                    gdk4::MemoryFormat::R8g8b8a8, //TODO: Premultiplied?
                    &my_texture.pixels().into(),
                    my_texture.width() as usize * 4,
                );

                let image = gtk4::Picture::for_paintable(&tex);

                image.set_hexpand(true);
                image.set_vexpand(true);

                let frame = gtk4::AspectFrame::builder()
                    .ratio(my_texture.width() as f32 / my_texture.height() as f32)
                    .child(&image)
                    .width_request(400)
                    .height_request(400)
                    .build();
                list_item.set_child(Some(&frame));
            }
        });

        let texture_count = document_arc.lock().unwrap().model.textures.len();
        let mut items = vec![];
        for i in 0..texture_count {
            items.push(NavigationItem::new(Path::ModelTexture(i as u64)));
        }

        // We can't add the bindings to the list while we're holding the
        // document lock or we'll deadlock.
        let list_store = gio::ListStore::builder().build();
        list_store.extend_from_slice(&items);
        self.imp().texture_selection.set_model(Some(&list_store));
    }
}
