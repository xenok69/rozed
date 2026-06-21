use zed_extension_api as zed;

struct RozedExtension;

impl zed::Extension for RozedExtension {
    fn new() -> Self {
        RozedExtension
    }
}

zed::register_extension!(RozedExtension);
