use crate::math::debug::SvgDump;
use std::{fs, io, ops, path::PathBuf};

pub struct SvgDumpFile {
    verbosity: usize,
    counter: usize,
    path: PathBuf,
    current: SvgDump,
}

impl SvgDumpFile {
    /// Creates a new SvgDumpFile with the specified verbosity level and output path.
    /// To disable dumping, set verbosity to 0.
    pub fn new<P: Into<PathBuf>>(verbosity: usize, path: P) -> Self {
        Self {
            verbosity,
            counter: 0,
            path: path.into(),
            current: SvgDump::new(),
        }
    }

    pub fn is_enabled(&self, level: usize) -> bool {
        self.verbosity > level
    }

    pub fn next_version(&mut self) {
        self.counter += 1;
    }

    pub fn filename(&self, prefix: &str) -> PathBuf {
        let base_name = format!("{:03}_{}.svg", self.counter, prefix);
        self.path.join(&base_name)
    }

    pub fn write_current(&mut self, prefix: &str) -> io::Result<()> {
        if self.verbosity > 0 {
            fs::create_dir_all(&self.path)?;
            let filename = self.filename(prefix);
            let mut file = fs::File::create(&filename)?;
            self.current.write(&mut file)?;
        }
        self.current.clear();
        Ok(())
    }

    pub fn scope<'a>(&'a mut self, level: usize, prefix: &'a str) -> Option<SvgDumpFileScope<'a>> {
        self.next_version();
        if self.is_enabled(level) {
            Some(SvgDumpFileScope { dump_file: self, prefix })
        } else {
            None
        }
    }
}

impl ops::Deref for SvgDumpFile {
    type Target = SvgDump;

    fn deref(&self) -> &Self::Target {
        &self.current
    }
}

impl ops::DerefMut for SvgDumpFile {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.current
    }
}

pub struct SvgDumpFileScope<'a> {
    dump_file: &'a mut SvgDumpFile,
    prefix: &'a str,
}

impl<'a> Drop for SvgDumpFileScope<'a> {
    fn drop(&mut self) {
        if let Err(err) = self.dump_file.write_current(self.prefix) {
            log::error!(
                "Error writing SVG dump file [{}]: {}",
                self.dump_file.filename(&self.prefix).to_string_lossy(),
                err
            );
        }
    }
}

impl<'a> ops::Deref for SvgDumpFileScope<'a> {
    type Target = SvgDump;

    fn deref(&self) -> &Self::Target {
        &self.dump_file.current
    }
}

impl<'a> ops::DerefMut for SvgDumpFileScope<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.dump_file.current
    }
}
