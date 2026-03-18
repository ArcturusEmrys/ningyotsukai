use glib;
use gtk4;
use gtk4::CompositeTemplate;
use gtk4::prelude::*;
use gtk4::subclass::prelude::*;

use glib::subclass::InitializingObject;

use std::borrow::Cow;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use inox2d::params::ParamUuid;

use crate::document::Document;
use crate::ext::{JsonValueExt, StrExt};
use crate::navigation::{NavigationItem, Path};

#[derive(CompositeTemplate, Default)]
#[template(resource = "/live/arcturus/puppet-inspector/detail_views/param/inspector.ui")]
pub struct ParamInspectorImp {
    document: RefCell<Option<(Arc<Mutex<Document>>, ParamUuid)>>,

    #[template_child]
    name_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    uuid_label: TemplateChild<gtk4::Entry>,
    #[template_child]
    min_x_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    min_y_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    max_x_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    max_y_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    default_x_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    default_y_field: TemplateChild<gtk4::Entry>,
    #[template_child]
    is_vec2_check: TemplateChild<gtk4::CheckButton>,
    #[template_child]
    bindings_view: TemplateChild<gtk4::ColumnView>,
    #[template_child]
    bindings_selection: TemplateChild<gtk4::SingleSelection>,
    #[template_child]
    bound_node_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    bound_property_factory: TemplateChild<gtk4::SignalListItemFactory>,
    #[template_child]
    bound_value_factory: TemplateChild<gtk4::SignalListItemFactory>,
}

#[glib::object_subclass]
impl ObjectSubclass for ParamInspectorImp {
    const NAME: &'static str = "PIPuppetParamInspector";
    type Type = ParamInspector;
    type ParentType = gtk4::Grid;

    fn class_init(class: &mut Self::Class) {
        class.bind_template();
    }

    fn instance_init(obj: &InitializingObject<Self>) {
        obj.init_template();
    }
}

impl ObjectImpl for ParamInspectorImp {
    fn constructed(&self) {
        self.parent_constructed();
    }
}

impl WidgetImpl for ParamInspectorImp {}

impl GridImpl for ParamInspectorImp {}

glib::wrapper! {
    pub struct ParamInspector(ObjectSubclass<ParamInspectorImp>)
        @extends gtk4::Grid, gtk4::Widget,
        @implements gtk4::Buildable, gtk4::Orientable, gtk4::ConstraintTarget, gtk4::Accessible;
}

impl ParamInspector {
    pub fn new(document: Arc<Mutex<Document>>, param_uuid: ParamUuid) -> Self {
        let selfish: Self = glib::Object::builder().build();

        *selfish.imp().document.borrow_mut() = Some((document, param_uuid));
        selfish.bind();

        selfish
    }

    fn bind(&self) {
        let (document_arc, param_uuid) = self.imp().document.borrow().as_ref().unwrap().clone();

        self.imp().bound_node_factory.connect_setup(|_, _| {});

        let node_document = document_arc.clone();
        self.imp()
            .bound_node_factory
            .connect_bind(move |_, object| {
                let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                let nav_item = list_item.item().unwrap();
                let nav = nav_item.downcast_ref::<NavigationItem>().unwrap();
                let (param_id, bind_id) = nav.as_puppet_param_binding().unwrap();

                let label = gtk4::Label::builder().halign(gtk4::Align::Start).build();

                let gtkbox = gtk4::Box::builder()
                    .orientation(gtk4::Orientation::Horizontal)
                    .hexpand(true)
                    .build();

                gtkbox.append(&label);

                let document = node_document.lock().unwrap();
                let param = document
                    .model
                    .puppet
                    .params()
                    .iter()
                    .find(|(_, p)| p.uuid == param_id);
                let binding = param.and_then(|(_, p)| p.bindings.get(bind_id));
                let node = binding.and_then(|b| document.model.puppet.nodes().get_node(b.node));

                if let Some(node) = node {
                    label.set_text(&node.name.escape_nulls());

                    let button = gtk4::Button::builder()
                        .halign(gtk4::Align::End)
                        .icon_name("go-jump")
                        .action_name("doc.jump")
                        .action_target(&Path::PuppetNode(node.uuid.into()).to_variant())
                        .build();

                    gtkbox.append(&button);
                }

                list_item.set_child(Some(&gtkbox));
            });

        self.imp()
            .bound_property_factory
            .connect_setup(|_, object| {
                let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                list_item.set_child(Some(
                    &gtk4::Label::builder().halign(gtk4::Align::Start).build(),
                ));
            });

        let prop_document = document_arc.clone();
        self.imp()
            .bound_property_factory
            .connect_bind(move |_, object| {
                let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                let nav_item = list_item.item().unwrap();
                let nav = nav_item.downcast_ref::<NavigationItem>().unwrap();

                let document = prop_document.lock().unwrap();
                let bind_path = nav.as_json_path(&document).unwrap();
                let property = document
                    .puppet_json
                    .traverse_path(bind_path.as_path())
                    .and_then(|v| v.as_object())
                    .and_then(|o| o.get("param_name"))
                    .and_then(|s| s.as_str());

                if let Some(property) = property {
                    let label_child = list_item.child().unwrap();
                    let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

                    label.set_text(&property.escape_nulls());
                }
            });

        self.imp().bound_value_factory.connect_setup(|_, object| {
            let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
            list_item.set_child(Some(
                &gtk4::Label::builder().halign(gtk4::Align::Start).build(),
            ));
        });

        let val_document = document_arc.clone();
        self.imp()
            .bound_value_factory
            .connect_bind(move |_, object| {
                let list_item = object.downcast_ref::<gtk4::ListItem>().unwrap();
                let nav_item = list_item.item().unwrap();
                let nav = nav_item.downcast_ref::<NavigationItem>().unwrap();
                let (param_id, bind_id) = nav.as_puppet_param_binding().unwrap();

                let document = val_document.lock().unwrap();
                let param = document
                    .model
                    .puppet
                    .params()
                    .iter()
                    .find(|(_, p)| p.uuid == param_id);
                let binding = param.and_then(|(_, p)| p.bindings.get(bind_id));

                if let Some(binding) = binding {
                    let label_child = list_item.child().unwrap();
                    let label = label_child.downcast_ref::<gtk4::Label>().unwrap();

                    label.set_text(
                        &match &binding.values {
                            inox2d::params::BindingValues::Deform(_) => {
                                Cow::Borrowed("(Deform vertices...)")
                            }
                            v => format!("{:?}", v).into(),
                        }
                        .escape_nulls(),
                    );
                }
            });

        let mut bindings = vec![];
        {
            let document = document_arc.lock().unwrap();
            let (name, param) = document
                .puppet_data()
                .params()
                .iter()
                .find(|(_k, v)| v.uuid == param_uuid)
                .expect("valid param");

            self.imp()
                .name_field
                .buffer()
                .set_text(name.escape_nulls().as_ref());
            self.imp()
                .uuid_label
                .buffer()
                .set_text(&format!("{}", param.uuid.0));
            self.imp()
                .min_x_field
                .buffer()
                .set_text(&format!("{}", param.min.x));
            self.imp()
                .min_y_field
                .buffer()
                .set_text(&format!("{}", param.min.y));
            self.imp()
                .max_x_field
                .buffer()
                .set_text(&format!("{}", param.max.x));
            self.imp()
                .max_y_field
                .buffer()
                .set_text(&format!("{}", param.max.y));
            self.imp()
                .default_x_field
                .buffer()
                .set_text(&format!("{}", param.defaults.x));
            self.imp()
                .default_y_field
                .buffer()
                .set_text(&format!("{}", param.defaults.y));
            self.imp().is_vec2_check.set_active(param.is_vec2);

            for (index, _) in param.bindings.iter().enumerate() {
                bindings.push(NavigationItem::new(Path::PuppetParamBinding(
                    param_uuid.into(),
                    index as u64,
                )));
            }
        }

        // We can't add the bindings to the list while we're holding the
        // document lock or we'll deadlock.
        let list_store = gio::ListStore::builder().build();
        list_store.extend_from_slice(&bindings);
        self.imp().bindings_selection.set_model(Some(&list_store));
    }
}
