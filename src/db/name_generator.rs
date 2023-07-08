use rand::Rng;
use thiserror::Error as ThisError;

mod name_list;
use name_list::NAME_LIST;

#[derive(Debug, ThisError)]
#[error("Failed to generate name")]
pub struct NameGeneratorError;

#[derive(Clone)]
pub struct NameGenerator {
    length: usize,
}

impl NameGenerator {
    pub fn new() -> Self {
        Self { length: 5 }
    }

    pub async fn generate_name(&self) -> Result<String, NameGeneratorError> {
        let mut name = String::new();
        let mut rng = rand::thread_rng();

        for _ in 0..self.length {
            if !name.is_empty() {
                name.push(' ');
            }
            let part = NAME_LIST[rng.gen_range(0..NAME_LIST.len())];
            log::info!("{}", part);
            name.push_str(part);
        }
        Ok(name)
    }
}

#[cfg(test)]
mod test {
    use crate::db::NameGenerator;
    use shine_test::test;

    #[test]
    async fn gen_names() {
        let gen = NameGenerator::new();

        for i in 0..10 {
            log::info!("{}. {}", i, gen.generate_name().await.unwrap())
        }
    }
}
