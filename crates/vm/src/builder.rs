use {
    anyhow::Context,
    std::{fs::File, io::Cursor, path::Path},
    wasmi::{Engine, Instance, IntoFunc, Linker, Module, Store},
};

pub struct InstanceBuilder<S> {
    engine: Engine,
    module: Option<Module>,
    store:  Option<Store<S>>,
    linker: Option<Linker<S>>,
}

impl<S> Default for InstanceBuilder<S> {
    fn default() -> Self {
        Self {
            engine: Engine::default(),
            module: None,
            store:  None,
            linker: None,
        }
    }
}

impl<S> InstanceBuilder<S> {
    pub fn new(engine: Engine) -> Self {
        Self {
            engine,
            module: None,
            store:  None,
            linker: None,
        }
    }

    pub fn with_wasm_file(mut self, path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let mut file = File::open(path)?;
        self.module = Some(Module::new(&self.engine, &mut file)?);
        Ok(self)
    }

    pub fn with_wasm_bytes(mut self, bytes: impl AsRef<[u8]>) -> anyhow::Result<Self> {
        self.module = Some(Module::new(&self.engine, Cursor::new(bytes))?);
        Ok(self)
    }

    pub fn with_storage(mut self, store: S) -> Self {
        self.store = Some(Store::new(&self.engine, store));
        self.linker = Some(Linker::new(&self.engine));
        self
    }

    pub fn with_host_function<Params, Results>(
        mut self,
        name: &str,
        func: impl IntoFunc<S, Params, Results>,
    ) -> anyhow::Result<Self> {
        let mut linker = self.take_linker()?;
        linker.func_wrap("env", name, func)?;
        self.linker = Some(linker);

        Ok(self)
    }

    pub fn finalize(mut self) -> anyhow::Result<(Instance, Store<S>)> {
        let module = self.take_module()?;
        let mut store = self.take_store()?;
        let linker = self.take_linker()?;
        let instance = linker.instantiate(&mut store, &module)?.start(&mut store)?;

        Ok((instance, store))
    }

    fn take_module(&mut self) -> anyhow::Result<Module> {
        self.module.take().context("Module not yet initialized")
    }

    fn take_store(&mut self) -> anyhow::Result<Store<S>> {
        self.store.take().context("Store not yet initialized")
    }

    fn take_linker(&mut self) -> anyhow::Result<Linker<S>> {
        self.linker.take().context("Linker not yet initialized")
    }
}
