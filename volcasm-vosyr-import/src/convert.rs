//! Conversion of a loaded Vosyr project into a Volcasm [`Project`].

use volcasm_core::device::VOLCA_SAMPLE;
use volcasm_core::pattern::{self, Pattern};
use volcasm_core::project::{Project, Slot};

use crate::{Error, VosyrProject};

impl VosyrProject {
    /// Builds a Volcasm project: slot names and samples, the ten sequences
    /// unchanged. Slots whose sample cannot be loaded stay empty; the errors
    /// are returned alongside.
    pub fn to_project(&self) -> (Project, Vec<Error>) {
        let mut project = Project::new(&VOLCA_SAMPLE, self.name.clone());
        let mut errors = Vec::new();
        for record in self.index.used() {
            let slot = self.load_sample(record.slot).and_then(|sample| {
                project
                    .set_slot(
                        record.slot,
                        Slot {
                            name: record.name.clone(),
                            sample,
                        },
                    )
                    .map_err(|e| Error::Sample {
                        slot: record.slot,
                        path: self.dir.join("samples").join(&record.file_name),
                        message: e.to_string(),
                    })
            });
            if let Err(err) = slot {
                errors.push(err);
            }
        }
        for (i, data) in self.sequences.iter().enumerate() {
            let pattern = Pattern::from_bytes(&pattern::to_bytes(data))
                .expect("sequences are validated when loading");
            project
                .set_pattern(i, pattern)
                .expect("Vosyr has as many sequences as the device");
        }
        (project, errors)
    }
}
