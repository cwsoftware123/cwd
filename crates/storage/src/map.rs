use {
    crate::{Borsh, Bound, Encoding, MapKey, PathBuf, Prefix, RawKey},
    grug_types::{Order, StdError, StdResult, Storage},
    std::marker::PhantomData,
};

pub struct Map<'a, K, T, E: Encoding<T> = Borsh> {
    namespace: &'a [u8],
    key: PhantomData<K>,
    data: PhantomData<T>,
    encoding: PhantomData<E>,
}

impl<'a, K, T, E> Map<'a, K, T, E>
where
    E: Encoding<T>,
{
    pub const fn new(namespace: &'a str) -> Self {
        // TODO: add a maximum length for namespace
        // see comments of increment_last_byte function for rationale
        Self {
            namespace: namespace.as_bytes(),
            key: PhantomData,
            data: PhantomData,
            encoding: PhantomData,
        }
    }
}

impl<'a, K, T, E> Map<'a, K, T, E>
where
    K: MapKey,
    E: Encoding<T>,
{
    fn path(&self, key: K) -> PathBuf<T, E> {
        let mut raw_keys = key.raw_keys();
        let last_raw_key = raw_keys.pop();
        PathBuf::new(self.namespace, &raw_keys, last_raw_key.as_ref())
    }

    fn path_raw(&self, key_raw: &[u8]) -> PathBuf<T, E> {
        PathBuf::new(self.namespace, &[], Some(&RawKey::Ref(key_raw)))
    }

    fn no_prefix(&self) -> Prefix<K, T, E> {
        Prefix::new(self.namespace, &[])
    }

    pub fn prefix(&self, prefix: K::Prefix) -> Prefix<K::Suffix, T, E> {
        Prefix::new(self.namespace, &prefix.raw_keys())
    }

    pub fn is_empty(&self, storage: &dyn Storage) -> bool {
        self.keys_raw(storage, None, None, Order::Ascending)
            .next()
            .is_none()
    }

    pub fn has_raw(&self, storage: &dyn Storage, key_raw: &[u8]) -> bool {
        self.path_raw(key_raw).as_path().exists(storage)
    }

    pub fn has(&self, storage: &dyn Storage, key: K) -> bool {
        self.path(key).as_path().exists(storage)
    }

    pub fn may_load_raw(&self, storage: &dyn Storage, key_raw: &[u8]) -> Option<Vec<u8>> {
        self.path_raw(key_raw).as_path().may_load_raw(storage)
    }

    pub fn may_load(&self, storage: &dyn Storage, key: K) -> StdResult<Option<T>> {
        self.path(key).as_path().may_load(storage)
    }

    pub fn load_raw(&self, storage: &dyn Storage, key_raw: &[u8]) -> StdResult<Vec<u8>> {
        self.path_raw(key_raw).as_path().load_raw(storage)
    }

    pub fn load(&self, storage: &dyn Storage, key: K) -> StdResult<T> {
        self.path(key).as_path().load(storage)
    }

    pub fn save_raw(&self, storage: &mut dyn Storage, key_raw: &[u8], data_raw: &[u8]) {
        self.path_raw(key_raw).as_path().save_raw(storage, data_raw)
    }

    pub fn save(&self, storage: &mut dyn Storage, key: K, data: &T) -> StdResult<()> {
        self.path(key).as_path().save(storage, data)
    }

    pub fn remove_raw(&self, storage: &mut dyn Storage, key_raw: &[u8]) {
        self.path_raw(key_raw).as_path().remove(storage)
    }

    pub fn remove(&self, storage: &mut dyn Storage, key: K) {
        self.path(key).as_path().remove(storage)
    }

    pub fn update<A, Err>(
        &self,
        storage: &mut dyn Storage,
        key: K,
        action: A,
    ) -> Result<Option<T>, Err>
    where
        A: FnOnce(Option<T>) -> Result<Option<T>, Err>,
        Err: From<StdError>,
    {
        self.path(key).as_path().update(storage, action)
    }

    #[allow(clippy::type_complexity)]
    pub fn range_raw<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = (Vec<u8>, Vec<u8>)> + 'b> {
        self.no_prefix().range_raw(storage, min, max, order)
    }

    #[allow(clippy::type_complexity)]
    pub fn range<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'b> {
        self.no_prefix().range(storage, min, max, order)
    }

    pub fn keys_raw<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Vec<u8>> + 'b> {
        self.no_prefix().keys_raw(storage, min, max, order)
    }

    pub fn keys<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<K::Output>> + 'b> {
        self.no_prefix().keys(storage, min, max, order)
    }

    pub fn clear(
        &self,
        storage: &mut dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        limit: Option<usize>,
    ) {
        self.no_prefix().clear(storage, min, max, limit)
    }
}

// ----------------------------------- tests -----------------------------------

#[cfg(test)]
mod test {
    use {
        crate::Map,
        borsh::{BorshDeserialize, BorshSerialize},
        grug_types::MockStorage,
    };

    const FOO: Map<u64, Foo> = Map::new("foo");

    #[derive(BorshDeserialize, BorshSerialize, Debug, PartialEq, Eq)]
    struct Foo {
        name: String,
        surname: String,
    }

    impl Foo {
        pub fn new(name: &str, surname: &str) -> Self {
            Self {
                name: name.to_string(),
                surname: surname.to_string(),
            }
        }
    }

    fn setup_test() -> MockStorage {
        let mut storage = MockStorage::new();

        for (key, name, surname) in [
            (1, "name_1", "surname_1"),
            (2, "name_2", "surname_2"),
            (3, "name_3", "surname_3"),
            (4, "name_4", "surname_4"),
        ] {
            FOO.save(&mut storage, key, &Foo::new(name, surname))
                .unwrap();
        }

        storage
    }

    #[test]
    fn map_works() {
        let storage = setup_test();

        let first = FOO.load(&storage, 1).unwrap();
        assert_eq!(first, Foo::new("name_1", "surname_1"));
    }
}
