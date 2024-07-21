
#[derive(Default, Clone, Copy, poise::ChoiceParameter)]
pub enum Model {
    #[default]
    UI16,
}

pub struct RVCSong {
    model: Model,
    url: String,
}

impl RVCSong {
    pub fn new(model: Model, url: String) -> Self {
        Self {
            model,
            url,
        }
    }
}