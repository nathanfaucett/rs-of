use iroh::{
    RelayMode, RelayUrl,
    endpoint::{Builder, presets, presets::Preset},
};

pub struct WebPreset {
    relays: Vec<RelayUrl>,
}

impl WebPreset {
    pub fn new(relay_urls: &[String]) -> Result<Self, String> {
        let relays = relay_urls
            .iter()
            .map(|url| url.parse::<RelayUrl>().map_err(|error| error.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { relays })
    }
}

impl Preset for WebPreset {
    fn apply(self, builder: Builder) -> Builder {
        let builder = presets::N0.apply(builder);
        if self.relays.is_empty() {
            builder
        } else {
            builder.relay_mode(RelayMode::custom(self.relays))
        }
    }
}
