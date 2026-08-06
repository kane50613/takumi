use std::collections::HashMap;
use std::fmt::Debug;
use std::hash::Hash;
use std::marker::PhantomData;

use pdf_writer::types::ProcSet;
use pdf_writer::writers;
use pdf_writer::{Chunk, Dict, Finish, Name, Ref};

use crate::krilla::serialize::{Cacheable, SerializeContext};
use crate::krilla::util::NameExt;

pub(crate) trait Resource {
  fn new(ref_: Ref) -> Self;
  fn get_ref(&self) -> Ref;
  fn get_dict<'a>(resources: &'a mut writers::Resources) -> Dict<'a>;
  fn get_prefix() -> &'static str;
  fn get_mapper(b: &mut ResourceDictionaryBuilder) -> &mut ResourceMapper<Self>;
}

pub(crate) trait Resourceable: Cacheable {
  type Resource: Resource;
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct ExtGState(Ref);

impl Resource for ExtGState {
  fn new(ref_: Ref) -> Self {
    Self(ref_)
  }

  fn get_ref(&self) -> Ref {
    self.0
  }

  fn get_dict<'a>(resources: &'a mut writers::Resources) -> Dict<'a> {
    resources.ext_g_states()
  }

  fn get_prefix() -> &'static str {
    "g"
  }

  fn get_mapper(b: &mut ResourceDictionaryBuilder) -> &mut ResourceMapper<ExtGState> {
    &mut b.ext_g_states
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct ColorSpace(Ref);

impl Resource for ColorSpace {
  fn new(ref_: Ref) -> Self {
    Self(ref_)
  }

  fn get_ref(&self) -> Ref {
    self.0
  }

  fn get_dict<'a>(resources: &'a mut writers::Resources) -> Dict<'a> {
    resources.color_spaces()
  }

  fn get_prefix() -> &'static str {
    "c"
  }

  fn get_mapper(b: &mut ResourceDictionaryBuilder) -> &mut ResourceMapper<ColorSpace> {
    &mut b.color_spaces
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct Shading(Ref);

impl Resource for Shading {
  fn new(ref_: Ref) -> Self {
    Self(ref_)
  }

  fn get_ref(&self) -> Ref {
    self.0
  }

  fn get_dict<'a>(resources: &'a mut writers::Resources) -> Dict<'a> {
    resources.shadings()
  }

  fn get_prefix() -> &'static str {
    "s"
  }

  fn get_mapper(b: &mut ResourceDictionaryBuilder) -> &mut ResourceMapper<Shading> {
    &mut b.shadings
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct XObject(Ref);

impl Resource for XObject {
  fn new(ref_: Ref) -> Self {
    Self(ref_)
  }

  fn get_ref(&self) -> Ref {
    self.0
  }

  fn get_dict<'a>(resources: &'a mut writers::Resources) -> Dict<'a> {
    resources.x_objects()
  }

  fn get_prefix() -> &'static str {
    "x"
  }

  fn get_mapper(b: &mut ResourceDictionaryBuilder) -> &mut ResourceMapper<XObject> {
    &mut b.x_objects
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct Pattern(Ref);

impl Resource for Pattern {
  fn new(ref_: Ref) -> Self {
    Self(ref_)
  }

  fn get_ref(&self) -> Ref {
    self.0
  }

  fn get_dict<'a>(resources: &'a mut writers::Resources) -> Dict<'a> {
    resources.patterns()
  }

  fn get_prefix() -> &'static str {
    "p"
  }

  fn get_mapper(b: &mut ResourceDictionaryBuilder) -> &mut ResourceMapper<Pattern> {
    &mut b.patterns
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct Font(Ref);

impl Resource for Font {
  fn new(ref_: Ref) -> Self {
    Self(ref_)
  }

  fn get_ref(&self) -> Ref {
    self.0
  }

  fn get_dict<'a>(resources: &'a mut writers::Resources) -> Dict<'a> {
    resources.fonts()
  }

  fn get_prefix() -> &'static str {
    "f"
  }

  fn get_mapper(b: &mut ResourceDictionaryBuilder) -> &mut ResourceMapper<Font> {
    &mut b.fonts
  }
}

#[derive(Debug)]
pub(crate) struct ResourceDictionaryBuilder {
  pub(crate) color_spaces: ResourceMapper<ColorSpace>,
  pub(crate) ext_g_states: ResourceMapper<ExtGState>,
  pub(crate) patterns: ResourceMapper<Pattern>,
  pub(crate) x_objects: ResourceMapper<XObject>,
  pub(crate) shadings: ResourceMapper<Shading>,
  pub(crate) fonts: ResourceMapper<Font>,
}

impl ResourceDictionaryBuilder {
  pub(crate) fn new() -> Self {
    Self {
      color_spaces: ResourceMapper::new(),
      ext_g_states: ResourceMapper::new(),
      patterns: ResourceMapper::new(),
      x_objects: ResourceMapper::new(),
      shadings: ResourceMapper::new(),
      fonts: ResourceMapper::new(),
    }
  }

  pub(crate) fn register_resource<T>(&mut self, obj: T) -> String
  where
    T: Resource,
  {
    T::get_mapper(self).remap_with_name(obj.get_ref())
  }

  pub(crate) fn finish(self) -> ResourceDictionary {
    ResourceDictionary {
      color_spaces: self.color_spaces.into_resource_list(),
      ext_g_states: self.ext_g_states.into_resource_list(),
      patterns: self.patterns.into_resource_list(),
      x_objects: self.x_objects.into_resource_list(),
      shadings: self.shadings.into_resource_list(),
      fonts: self.fonts.into_resource_list(),
    }
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone)]
pub(crate) struct ResourceDictionary {
  pub(crate) color_spaces: ResourceList<ColorSpace>,
  pub(crate) ext_g_states: ResourceList<ExtGState>,
  pub(crate) patterns: ResourceList<Pattern>,
  pub(crate) x_objects: ResourceList<XObject>,
  pub(crate) shadings: ResourceList<Shading>,
  pub(crate) fonts: ResourceList<Font>,
}

impl Default for ResourceDictionary {
  fn default() -> Self {
    Self {
      color_spaces: ResourceList::empty(),
      ext_g_states: ResourceList::empty(),
      patterns: ResourceList::empty(),
      x_objects: ResourceList::empty(),
      shadings: ResourceList::empty(),
      fonts: ResourceList::empty(),
    }
  }
}

pub(crate) type ResourceNumber = u32;

impl ResourceDictionary {
  pub fn to_pdf_resources<T>(
    &self,
    parent: &mut T,
    sc: &mut SerializeContext,
    resources_chunk: &mut Chunk,
  ) where
    T: ResourcesExt,
  {
    let write_proc_sets = !sc.serialize_settings().pdf_version().deprecates_proc_sets();
    let has_resource_entries = self.color_spaces.len() > 0
      || self.ext_g_states.len() > 0
      || self.patterns.len() > 0
      || self.x_objects.len() > 0
      || self.shadings.len() > 0
      || self.fonts.len() > 0;

    if !write_proc_sets && !has_resource_entries {
      // `Resources` dictionary is mandatory (or rather, it's mandatory if
      // there isn't a parent resource dictionary to inherit from, but we just
      // assume it is always mandatory), so just write an empty one as a direct
      // object instead of an indirect object, for aesthetic reasons.
      parent.resources().finish();
      return;
    }

    let resources_ref = sc.new_ref();
    let mut resources = resources_chunk
      .indirect(resources_ref)
      .start::<writers::Resources>();
    if write_proc_sets {
      resources.proc_sets([
        ProcSet::Pdf,
        ProcSet::Text,
        ProcSet::ImageColor,
        ProcSet::ImageGrayscale,
      ]);
    }
    write_resource_type::<ColorSpace>(&mut resources, &self.color_spaces);
    write_resource_type::<ExtGState>(&mut resources, &self.ext_g_states);
    write_resource_type::<Pattern>(&mut resources, &self.patterns);
    write_resource_type::<XObject>(&mut resources, &self.x_objects);
    write_resource_type::<Shading>(&mut resources, &self.shadings);
    write_resource_type::<Font>(&mut resources, &self.fonts);
    parent.set_resources(resources_ref);
  }
}

fn write_resource_type<T>(resources: &mut writers::Resources, resource_list: &ResourceList<T>)
where
  T: Resource,
{
  if resource_list.len() > 0 {
    let mut dict = T::get_dict(resources);

    for (name, entry) in resource_list.get_entries() {
      dict.pair(name.to_pdf_name(), entry);
    }

    dict.finish();
  }
}

#[derive(Debug, Eq, PartialEq, Hash, Clone, Default)]
pub(crate) struct ResourceList<V> {
  entries: Vec<Ref>,
  phantom: PhantomData<V>,
}

impl<T> ResourceList<T>
where
  T: Resource,
{
  pub(crate) fn empty() -> ResourceList<T> {
    Self {
      entries: vec![],
      phantom: Default::default(),
    }
  }

  pub(crate) fn len(&self) -> u32 {
    self.entries.len() as u32
  }

  fn name_from_number(num: ResourceNumber) -> String {
    format!("{}{}", T::get_prefix(), num)
  }

  pub(crate) fn get_entries(&self) -> impl Iterator<Item = (String, Ref)> + '_ {
    self
      .entries
      .iter()
      .enumerate()
      .map(|(i, r)| (Self::name_from_number(i as ResourceNumber), *r))
  }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct ResourceMapper<T: ?Sized> {
  forward: Vec<Ref>,
  backward: HashMap<Ref, ResourceNumber>,
  phantom: PhantomData<T>,
}

impl<T> ResourceMapper<T>
where
  T: Resource,
{
  pub(crate) fn new() -> Self {
    Self {
      forward: Vec::new(),
      backward: HashMap::new(),
      phantom: PhantomData,
    }
  }

  pub(crate) fn remap(&mut self, ref_: Ref) -> ResourceNumber {
    let forward = &mut self.forward;
    let backward = &mut self.backward;

    *backward.entry(ref_).or_insert_with(|| {
      let old = forward.len();
      forward.push(ref_);
      old as ResourceNumber
    })
  }

  pub(crate) fn remap_with_name(&mut self, ref_: Ref) -> String {
    Self::name_from_number(self.remap(ref_))
  }

  fn name_from_number(num: ResourceNumber) -> String {
    format!("{}{}", T::get_prefix(), num)
  }

  pub(crate) fn into_resource_list(self) -> ResourceList<T> {
    ResourceList {
      entries: self.forward,
      phantom: Default::default(),
    }
  }
}

pub(crate) trait ResourcesExt {
  fn resources(&mut self) -> writers::Resources<'_>;
  fn set_resources(&mut self, resources_ref: Ref);
}

impl ResourcesExt for writers::FormXObject<'_> {
  fn resources(&mut self) -> writers::Resources<'_> {
    self.resources()
  }

  fn set_resources(&mut self, resources_ref: Ref) {
    self.pair(Name(b"Resources"), resources_ref);
  }
}

impl ResourcesExt for writers::TilingPattern<'_> {
  fn resources(&mut self) -> writers::Resources<'_> {
    self.resources()
  }

  fn set_resources(&mut self, resources_ref: Ref) {
    self.pair(Name(b"Resources"), resources_ref);
  }
}

impl ResourcesExt for writers::Type3Font<'_> {
  fn resources(&mut self) -> writers::Resources<'_> {
    self.resources()
  }

  fn set_resources(&mut self, resources_ref: Ref) {
    self.pair(Name(b"Resources"), resources_ref);
  }
}

impl ResourcesExt for writers::Pages<'_> {
  fn resources(&mut self) -> writers::Resources<'_> {
    self.resources()
  }

  fn set_resources(&mut self, resources_ref: Ref) {
    self.pair(Name(b"Resources"), resources_ref);
  }
}

impl ResourcesExt for writers::Page<'_> {
  fn resources(&mut self) -> writers::Resources<'_> {
    self.resources()
  }

  fn set_resources(&mut self, resources_ref: Ref) {
    self.pair(Name(b"Resources"), resources_ref);
  }
}
