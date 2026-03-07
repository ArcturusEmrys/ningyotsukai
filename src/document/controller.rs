use gio;
use glib;
use gtk4;

use gio::prelude::*;
use glib::subclass::InitializingObject;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use std::cell::RefCell;

use std::sync::{Arc, Mutex};

use crate::document::Document;
use crate::navigation::{NavigationItem, Path, Section};
use crate::string_ext::StrExt;

/// For some reason, glib-rs does not support mutating private/impl structs.
/// Hence the mutability hack.
#[derive(Default)]
pub struct DocumentControllerState {
    open_doc: Option<Arc<Mutex<Document>>>,
    navigation_tree: Option<gtk4::TreeListModel>,
    json_tree: Option<gtk4::TreeListModel>,
    root_nav_list: Option<gio::ListStore>,
    root_json_list: Option<gio::ListStore>,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/document.ui")]
pub struct DocumentControllerImp {
    #[template_child]
    navigation_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    navigation_selection: TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    json_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    json_selection: TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    detail_view: TemplateChild<gtk4::ScrolledWindow>,
    #[template_child]
    tabs: TemplateChild<gtk4::Notebook>,
    state: RefCell<DocumentControllerState>,
}

#[glib::object_subclass]
impl ObjectSubclass for DocumentControllerImp {
    const NAME: &'static str = "PIDocumentController";
    type Type = DocumentController;
    type ParentType = gtk4::Box;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for DocumentControllerImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for DocumentControllerImp {}

impl BoxImpl for DocumentControllerImp {}

glib::wrapper! {
    pub struct DocumentController(ObjectSubclass<DocumentControllerImp>)
        @extends gtk4::Box, gtk4::Widget,
        @implements gtk4::Accessible, gtk4::Buildable, gtk4::ConstraintTarget;
}

impl DocumentController {
    pub fn new(open_doc: Arc<Mutex<Document>>) -> Self {
        let selfish: DocumentController = glib::Object::builder().build();

        selfish.imp().state.borrow_mut().open_doc = Some(open_doc.clone());
        selfish.populate_navigation();

        selfish
    }

    pub fn populate_navigation(&self) {
        let mut state = self.imp().state.borrow_mut();
        if state.root_nav_list.is_none() {
            state.root_nav_list = Some(gio::ListStore::builder().build());
        }
        if state.root_json_list.is_none() {
            state.root_json_list = Some(gio::ListStore::builder().build());
        }

        let root_nav_list = state.root_nav_list.clone().unwrap();

        if state.navigation_tree.is_none() {
            let callback_self = self.clone();
            let navigation_tree =
                gtk4::TreeListModel::new(root_nav_list.clone(), false, false, move |node| {
                    let nav = node
                        .clone()
                        .downcast::<NavigationItem>()
                        .expect("our own child");
                    let state = callback_self.imp().state.borrow();
                    let document = state.open_doc.as_ref();

                    if let Some(document) = document {
                        nav.child_list(&document.lock().unwrap())
                    } else {
                        None
                    }
                });
            state.navigation_tree = Some(navigation_tree.clone());

            self.imp()
                .navigation_selection
                .set_model(Some(&navigation_tree));
        }

        let root_json_list = state.root_json_list.clone().unwrap();

        if state.json_tree.is_none() {
            let callback_self = self.clone();
            let json_tree =
                gtk4::TreeListModel::new(root_json_list.clone(), false, false, move |node| {
                    let nav = node
                        .clone()
                        .downcast::<NavigationItem>()
                        .expect("our own child");
                    let state = callback_self.imp().state.borrow();
                    let document = state.open_doc.as_ref();

                    if let Some(document) = document {
                        nav.child_list(&document.lock().unwrap())
                    } else {
                        None
                    }
                });
            state.json_tree = Some(json_tree.clone());

            self.imp().json_selection.set_model(Some(&json_tree));
        }

        let mut root_json = vec![NavigationItem::new(Path::PuppetJson(Vec::new()))];
        for (index, _) in state
            .open_doc
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .vendors()
            .iter()
            .enumerate()
        {
            root_json.push(NavigationItem::new(Path::VendorJson(
                index as u64,
                Vec::new(),
            )))
        }

        root_nav_list.extend_from_slice(&[
            NavigationItem::new(Path::Section(Section::PuppetMeta)),
            NavigationItem::new(Path::Section(Section::PuppetPhysics)),
            NavigationItem::new(Path::Section(Section::PuppetNode)),
            NavigationItem::new(Path::Section(Section::PuppetParams)),
            NavigationItem::new(Path::Section(Section::ModelTextures)),
            NavigationItem::new(Path::Section(Section::VendorData)),
        ]);

        root_json_list.extend_from_slice(root_json.as_slice());

        drop(state);

        self.connect_factory(
            self.imp().navigation_factory.clone(),
            &self.imp().navigation_selection,
        );
        self.connect_factory(self.imp().json_factory.clone(), &self.imp().json_selection);

        let json_selection = self.imp().json_selection.clone();
        let callback_self = self.clone();
        json_selection.connect_selection_changed(move |model, position, count| {
            for position in position..position + count {
                if !model.is_selected(position) {
                    continue;
                }

                let tree_row = model.item(position);
                if let Some(tree_row) = tree_row {
                    let item = tree_row
                        .downcast::<gtk4::TreeListRow>()
                        .expect("tree row")
                        .item();
                    if let Some(item) = item {
                        let item = item.downcast::<NavigationItem>().expect("nav item");
                        callback_self.populate_detail(item);
                    }
                }
            }
        });

        self.populate_detail(NavigationItem::new(Path::Section(Section::PuppetMeta)));

        let notebook_self = self.clone();
        self.imp()
            .tabs
            .connect_switch_page(move |_note, _page, page_num| {
                let model = match page_num {
                    0 => &notebook_self.imp().navigation_selection, //Resources page
                    1 => &notebook_self.imp().json_selection,       //JSON page
                    2 => {
                        //OpenGL render page
                        return notebook_self
                            .populate_detail(NavigationItem::new(Path::RenderPreview));
                    }
                    unk => panic!("Unknown page {}", unk),
                };

                if let Some((_, selected_id)) = gtk4::BitsetIter::init_first(&model.selection()) {
                    let tree_row = model.item(selected_id).expect("valid selection");
                    let item = tree_row
                        .downcast::<gtk4::TreeListRow>()
                        .expect("tree row")
                        .item()
                        .expect("nav item obj")
                        .downcast::<NavigationItem>()
                        .expect("nav item");

                    notebook_self.populate_detail(item);
                }
            });
    }

    fn connect_factory(
        &self,
        factory: gtk4::SignalListItemFactory,
        selection: &gtk4::SingleSelection,
    ) {
        let factory_callback_self = self.clone();

        factory.connect_setup(|_factory, list_item| {
            let label = gtk4::Label::new(None);
            let tree_expander = gtk4::TreeExpander::builder().build();

            tree_expander.set_child(Some(&label));

            let list_item = list_item
                .downcast_ref::<gtk4::ListItem>()
                .expect("list item");

            list_item.set_child(Some(&tree_expander));
            list_item.set_property("focusable", false);
        });

        factory.connect_bind(move |_factory, list_item| {
            let list_item = list_item
                .downcast_ref::<gtk4::ListItem>()
                .expect("list item");

            let mut maybe_nav = list_item.item().expect("list items to have a child");
            let mut tree_list_row = None;
            while maybe_nav.clone().downcast::<NavigationItem>().is_err() {
                let tlr = maybe_nav
                    .downcast::<gtk4::TreeListRow>()
                    .expect("valid child list item");
                tree_list_row = Some(tlr.clone());
                if let Some(child) = tlr.item() {
                    maybe_nav = child;
                } else {
                    panic!("No navigation child!");
                }
            }

            let nav = maybe_nav
                .downcast::<NavigationItem>()
                .expect("our own child");
            let tree_item = list_item
                .child()
                .and_downcast::<gtk4::TreeExpander>()
                .expect("our own tree expander");

            tree_item.set_list_row(tree_list_row.as_ref());

            let label = tree_item
                .child()
                .and_downcast::<gtk4::Label>()
                .expect("our own label");
            let state = factory_callback_self.imp().state.borrow();

            if let Some(document) = state.open_doc.as_ref() {
                label.set_label(nav.name(&document.lock().unwrap()).escape_nulls().as_ref());
            } else {
                label.set_label("Wot! No document?");
            }
        });

        let callback_self = self.clone();
        selection.connect_selection_changed(move |model, position, count| {
            for position in position..position + count {
                if !model.is_selected(position) {
                    continue;
                }

                let tree_row = model.item(position);
                if let Some(tree_row) = tree_row {
                    let item = tree_row
                        .downcast::<gtk4::TreeListRow>()
                        .expect("tree row")
                        .item();
                    if let Some(item) = item {
                        let item = item.downcast::<NavigationItem>().expect("nav item");
                        callback_self.populate_detail(item);
                    }
                }
            }
        });
    }

    fn populate_detail(&self, item: NavigationItem) {
        let detail_view = self.imp().detail_view.clone();
        let document = self.imp().state.borrow().open_doc.clone().unwrap();

        detail_view.set_child(Some(&item.child_inspector(document)));
    }

    pub fn jump_to_path(&self, item: NavigationItem) {
        let notebook_page = item.notebook_page();
        let path = item.as_path();

        self.imp().tabs.set_current_page(Some(notebook_page));

        let tree_selection = match notebook_page {
            0 => self.imp().navigation_selection.clone(),
            1 => self.imp().json_selection.clone(),
            _ => return,
        };

        // Literally open the entire tree until we run out of shit to open.
        // I hate that we have to do this, but GTK's fancy tree list model
        // gives us no other choice. The entire tree has to be forcibly
        // materialized before we can search it.
        //
        // (Well, I COULD work backwards from the JSON but that would be even
        // dumber than this, and it would tie my hands more with regards to
        // application design.)
        let mut newly_opened_rows = vec![];
        let mut our_row_and_precursors = vec![];
        loop {
            let mut did_open_a_row = false;
            for item in tree_selection.iter::<glib::Object>() {
                let item_row = item.unwrap().downcast::<gtk4::TreeListRow>().unwrap();
                let item_row_item = item_row
                    .item()
                    .unwrap()
                    .downcast::<NavigationItem>()
                    .unwrap();

                if item_row_item.as_path() == path {
                    // Ladies and gentlemen, we got 'em.
                    let mut parent = item_row.parent();

                    our_row_and_precursors.push(item_row);
                    while parent.is_some() {
                        let parent_unwrap = parent.unwrap();
                        let gp = parent_unwrap.parent();

                        our_row_and_precursors.push(parent_unwrap);
                        parent = gp;
                    }
                    break;
                }

                if item_row.is_expandable() && !item_row.is_expanded() {
                    item_row.set_expanded(true);

                    // We have to break here as we just invalidated our
                    // iterator. Yes, GTK-rs reinvented iterator invalidation
                    // in Rust. Somehow.
                    did_open_a_row = true;
                    newly_opened_rows.push(item_row);
                    break;
                }
            }

            if !did_open_a_row {
                break;
            }
        }

        // Opening the entire tree is going to be VERY disorienting to the
        // user, so let's close everything we opened.
        for row in newly_opened_rows {
            if our_row_and_precursors.contains(&row) {
                continue;
            }

            row.set_expanded(false);
        }

        // FINALLY, we can do a proper linear scan for our row in the
        // selection model.
        let mut desired_index = None;
        for (linear_index, object) in tree_selection.iter::<glib::Object>().enumerate() {
            let tree_row = object.unwrap().downcast::<gtk4::TreeListRow>().unwrap();
            let path_item = tree_row
                .item()
                .unwrap()
                .downcast::<NavigationItem>()
                .unwrap();

            if path_item.as_path() == path {
                desired_index = Some(linear_index);
                break;
            }
        }

        if let Some(desired_index) = desired_index {
            tree_selection.set_selected(desired_index as u32);
        }
    }
}
