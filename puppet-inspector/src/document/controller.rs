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
use crate::navbar::NavbarController;
use crate::navigation::{NavigationItem, Path, Section};
use crate::render_preview::InoxRenderPreview;
use ningyo_extensions::prelude::*;

/// For some reason, glib-rs does not support mutating private/impl structs.
/// Hence the mutability hack.
#[derive(Default)]
pub struct DocumentControllerState {
    open_doc: Option<Arc<Mutex<Document>>>,
    navigation_tree: Option<gtk4::TreeListModel>,
    json_tree: Option<gtk4::TreeListModel>,
    root_nav_list: Option<gio::ListStore>,
    root_json_list: Option<gio::ListStore>,
    doc: Option<gio::SimpleActionGroup>,
    history: Vec<Path>,
    current: Option<Path>,
    future: Vec<Path>,

    // Meta note: all of the following flags exist to work around various
    // consequences of how GTK fires events. Namely, user interaction as well
    // as our own code setting the same values fires the same event and there's
    // no built-in way to tell which. We also rely on this doing the same thing
    // so that we can just change tabs or selection to actually jump between
    // pages of the UI.
    /// Indicates TRUE if the current tab switch is being caused by program
    /// code instead of a user action. If TRUE, disables "automatic jump to
    /// JSON" so that the back/fwd buttons work.
    no_automatic_jump_on_tab_switch: bool,

    /// Indicates TRUE if the current page switch is being caused by back/fwd
    /// action, in which case the future stack should not automatically be
    /// cleared. Otherwise, it DOES get cleared, because user navigation to new
    /// pages should NOT preserve the forward stack
    preserve_future: bool,

    /// Indicates TRUE if an event is currently being handled; if so, do
    /// nothing.
    ///
    /// The following functions ignore reentrant calls from one another:
    ///
    /// * tabs.switch_page's event handler
    /// * json/nav selection.selection_changed's event handler(s)
    /// * jump_back
    /// * jump_fwd
    /// * jump_up
    /// * jump_to
    fuck_reentrancy: bool,
}

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/document/controller.ui")]
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
    #[template_child]
    navbar: TemplateChild<NavbarController>,

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
        selfish
            .imp()
            .navbar
            .set_document_and_parent(open_doc, selfish.clone());
        selfish.bind_actions();
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

        self.append_history(Path::Section(Section::PuppetMeta));
        self.populate_detail(NavigationItem::new(Path::Section(Section::PuppetMeta)));

        self.imp().tabs.connect_switch_page({
            let switch_self = self.downgrade();
            move |_note, _page, page_num| {
                if let Some(switch_self) = switch_self.upgrade() {
                    if switch_self.imp().state.borrow().fuck_reentrancy {
                        return;
                    }

                    switch_self.imp().state.borrow_mut().fuck_reentrancy = true;

                    let state = switch_self.imp().state.borrow_mut();
                    let document = state.open_doc.clone().unwrap();
                    let document = document.lock().unwrap();
                    if !state.no_automatic_jump_on_tab_switch && page_num == 1 && false {
                        // Automatic Jump to JSON
                        if let Some(json_path) = state
                            .current
                            .as_ref()
                            .and_then(|c| c.as_json_path(&document))
                        {
                            drop(document);
                            drop(state);

                            let path: Path = json_path.into();

                            switch_self.select_path_on_tree(path.clone());
                            switch_self.append_history(path.clone());
                            switch_self.populate_detail(NavigationItem::new(path));
                            switch_self.imp().state.borrow_mut().fuck_reentrancy = false;

                            return;
                        }
                    }

                    drop(document);
                    drop(state);

                    let model = match page_num {
                        0 => &switch_self.imp().navigation_selection, //Resources page
                        1 => &switch_self.imp().json_selection,       //JSON page
                        unk => panic!("Unknown page {}", unk),
                    };

                    if let Some((_, selected_id)) = gtk4::BitsetIter::init_first(&model.selection())
                    {
                        let tree_row = model.item(selected_id).expect("valid selection");
                        let item = tree_row
                            .downcast::<gtk4::TreeListRow>()
                            .expect("tree row")
                            .item()
                            .expect("nav item obj")
                            .downcast::<NavigationItem>()
                            .expect("nav item");

                        switch_self.select_path_on_tree(item.as_path());
                        switch_self.append_history(item.as_path());
                        switch_self.populate_detail(item);
                    }

                    switch_self.imp().state.borrow_mut().fuck_reentrancy = false;
                }
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
            if callback_self.imp().state.borrow().fuck_reentrancy {
                return;
            }

            callback_self.imp().state.borrow_mut().fuck_reentrancy = true;

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
                        callback_self.select_path_on_tabs(item.as_path());
                        callback_self.append_history(item.as_path());
                        callback_self.populate_detail(item);
                    }
                }
            }

            callback_self.imp().state.borrow_mut().fuck_reentrancy = false;
        });
    }

    fn append_history(&self, path: Path) {
        let mut state = self.imp().state.borrow_mut();
        if let Some(prior) = state.current.take() {
            state.history.push(prior);
        }

        if !state.preserve_future {
            state.future.clear();
        }
        state.current = Some(path.clone());
    }

    fn populate_detail(&self, item: NavigationItem) {
        let detail_view = self.imp().detail_view.clone();
        let state = self.imp().state.borrow_mut();
        let path = item.as_path();
        self.imp().navbar.set_path(path);

        state
            .doc
            .as_ref()
            .unwrap()
            .lookup_action("back")
            .unwrap()
            .set_property("enabled", state.history.len() > 0);
        state
            .doc
            .as_ref()
            .unwrap()
            .lookup_action("fwd")
            .unwrap()
            .set_property("enabled", state.future.len() > 0);

        let document = state.open_doc.clone().unwrap();

        drop(state);
        detail_view.set_child(Some(&item.child_inspector(document)));
    }

    pub fn bind_actions(&self) {
        let doc = gio::SimpleActionGroup::new();

        let doc_controller_jump = self.clone();
        let doc_controller_back = self.clone();
        let doc_controller_fwd = self.clone();
        let doc_controller_preview = self.clone();
        doc.add_action_entries([
            gio::ActionEntry::builder("jump")
                .activate(move |_, _, variant| {
                    if let Some(path) = variant.and_then(|v| Path::from_variant(v)) {
                        doc_controller_jump.jump_to(path);
                    }
                })
                .parameter_type(Some(&Path::static_variant_type()))
                .build(),
            gio::ActionEntry::builder("back")
                .activate(move |_, _, _| {
                    doc_controller_back.jump_back();
                })
                .build(),
            gio::ActionEntry::builder("fwd")
                .activate(move |_, _, _| {
                    doc_controller_fwd.jump_fwd();
                })
                .build(),
            gio::ActionEntry::builder("preview")
                .activate(move |_, _, _| {
                    let document = doc_controller_preview
                        .imp()
                        .state
                        .borrow()
                        .open_doc
                        .as_ref()
                        .unwrap()
                        .clone();
                    let rp_window = InoxRenderPreview::new(document);

                    rp_window.present();
                })
                .build(),
        ]);

        doc.lookup_action("back")
            .unwrap()
            .set_property("enabled", false);
        doc.lookup_action("fwd")
            .unwrap()
            .set_property("enabled", false);

        self.imp().state.borrow_mut().doc = Some(doc.clone());

        self.connect_realize(move |selfpoi| {
            selfpoi
                .window()
                .unwrap()
                .insert_action_group("doc", Some(&doc));
        });
    }

    fn jump_back(&self) {
        let mut state = self.imp().state.borrow_mut();
        if state.fuck_reentrancy {
            return;
        }

        state.fuck_reentrancy = true;

        let back = state.history.pop();

        if let Some(back) = back {
            // By clearing current here we ensure populate_detail does not put
            // it back on the history stack
            if let Some(current) = state.current.take() {
                state.future.push(current);
            }

            // We need to tell populate_detail not to obliterate the future
            // stack we just made
            state.preserve_future = true;

            drop(state);
            self.jump_to_inner(back);

            self.imp().state.borrow_mut().preserve_future = false;
        }

        self.imp().state.borrow_mut().fuck_reentrancy = false;
    }

    fn jump_fwd(&self) {
        let mut state = self.imp().state.borrow_mut();
        if state.fuck_reentrancy {
            return;
        }

        state.fuck_reentrancy = true;

        let fwd = state.future.pop();

        // We should, obviously, be allowed to go forward more than once.
        state.preserve_future = true;

        drop(state);
        if let Some(fwd) = fwd {
            self.jump_to_inner(fwd);
        }

        self.imp().state.borrow_mut().preserve_future = false;
        self.imp().state.borrow_mut().fuck_reentrancy = false;
    }

    pub fn jump_to(&self, path: Path) {
        let mut state = self.imp().state.borrow_mut();

        if state.fuck_reentrancy {
            return;
        }

        state.fuck_reentrancy = true;

        drop(state);
        self.jump_to_inner(path);

        self.imp().state.borrow_mut().fuck_reentrancy = false;
    }

    pub fn jump_up(&self) {
        let mut state = self.imp().state.borrow_mut();

        if state.fuck_reentrancy {
            return;
        }

        state.fuck_reentrancy = true;
        if let Some(current) = state.current.as_ref() {
            let parent = current.parent(&*state.open_doc.as_ref().unwrap().lock().unwrap());
            if let Some(parent) = parent {
                drop(state);
                self.jump_to_inner(parent);
            }
        }

        self.imp().state.borrow_mut().fuck_reentrancy = false;
    }

    fn jump_to_inner(&self, path: Path) {
        self.select_path_on_tabs(path.clone());
        self.select_path_on_tree(path.clone());
        self.append_history(path.clone());
        self.populate_detail(NavigationItem::new(path));
    }

    /// Select a particular path on the tabs WITHOUT selecting tree items.
    fn select_path_on_tabs(&self, path: Path) {
        let notebook_page = path.notebook_page();

        if notebook_page != self.imp().tabs.current_page().unwrap_or(u32::MAX) {
            // Disable Automatic Jump To JSON behavior since the current page
            // change signal triggers for both back/fwd interactions and the
            // user switching pages
            self.imp()
                .state
                .borrow_mut()
                .no_automatic_jump_on_tab_switch = true;
            self.imp().tabs.set_current_page(Some(notebook_page));
            self.imp()
                .state
                .borrow_mut()
                .no_automatic_jump_on_tab_switch = false;
        }
    }

    /// Select a particular path on the tree WITHOUT switching tabs.
    fn select_path_on_tree(&self, path: Path) {
        let tree_selection = match path.notebook_page() {
            0 => self.imp().navigation_selection.clone(),
            1 => self.imp().json_selection.clone(),
            _ => return,
        };

        // Create a list of list rows to open.
        // These have to be opened in reverse order (highest first), and we
        // assume the highest option in the list is a root item.
        let mut list_of_things_to_open = vec![path.clone()];
        {
            let state = self.imp().state.borrow();
            let document_ref = state.open_doc.as_ref().unwrap();
            let document = document_ref.lock().unwrap();

            loop {
                if let Some(path) = list_of_things_to_open.last().unwrap().parent(&document) {
                    list_of_things_to_open.push(path);
                } else {
                    break;
                }
            }
        }

        let mut desired_index = None;
        loop {
            if let Some(path) = list_of_things_to_open.pop() {
                for (linear_index, object) in tree_selection.iter::<glib::Object>().enumerate() {
                    let tree_row = object.unwrap().downcast::<gtk4::TreeListRow>().unwrap();
                    let path_item = tree_row
                        .item()
                        .unwrap()
                        .downcast::<NavigationItem>()
                        .unwrap();

                    if path_item.as_path() == path {
                        desired_index = Some(linear_index);
                        if tree_row.is_expandable() && !tree_row.is_expanded() {
                            tree_row.set_expanded(true);
                        }

                        // Expanding the row invalidates our iterator, so we
                        // have to loop back around and start over!
                        break;
                    }
                }
            } else {
                break;
            }
        }

        if let Some(desired_index) = desired_index {
            tree_selection.set_selected(desired_index as u32);
        }
    }
}
