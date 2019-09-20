pub mod addons;

// Determine docker files and architectures
pub(crate) struct BuildInstruction {
    pub(crate) filename: String,
    pub(crate) arch: String,
    pub(crate) image_name: String,
    pub(crate) build: bool,
    pub(crate) uploaded: bool,
    pub(crate) image_size: i64,
}