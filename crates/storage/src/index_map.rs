use {
    crate::{Borsh, Bound, Encoding, Map, MapKey, Prefix},
    grug_types::{Order, Record, StdError, StdResult, Storage},
};

pub trait IndexList<T> {
    fn get_indexes(&self) -> Box<dyn Iterator<Item = &'_ dyn Index<T>> + '_>;
}

pub trait Index<T> {
    fn save(&self, store: &mut dyn Storage, pk: &[u8], data: &T) -> StdResult<()>;
    fn remove(&self, store: &mut dyn Storage, pk: &[u8], old_data: &T) -> StdResult<()>;
}

pub struct IndexedMap<'a, K, T, I, E: Encoding<T> = Borsh> {
    pk_namespace: &'a [u8],
    primary: Map<'a, K, T, E>,
    /// This is meant to be read directly to get the proper types, like:
    /// map.idx.owner.items(...)
    pub idx: I,
}

impl<'a, K, T, I, E: Encoding<T>> IndexedMap<'a, K, T, I, E>
where
    E: Encoding<T>,
    K: MapKey,
    I: IndexList<T>,
{
    pub const fn new(pk_namespace: &'static str, indexes: I) -> Self {
        IndexedMap {
            pk_namespace: pk_namespace.as_bytes(),
            primary: Map::new(pk_namespace),
            idx: indexes,
        }
    }

    pub fn has(&self, storage: &dyn Storage, k: K) -> bool {
        self.primary.has(storage, k)
    }
}

impl<'a, K, T, I, E: Encoding<T>> IndexedMap<'a, K, T, I, E>
where
    K: MapKey,
    I: IndexList<T>,
    E: Encoding<T>,
{
    pub fn is_empty(&self, storage: &dyn Storage) -> bool {
        self.no_prefix()
            .keys_raw(storage, None, None, Order::Ascending)
            .next()
            .is_none()
    }

    fn no_prefix(&self) -> Prefix<K, T, E> {
        Prefix::new(self.pk_namespace, &[])
    }

    pub fn prefix(&self, prefix: K::Prefix) -> Prefix<K::Suffix, T, E> {
        Prefix::new(self.pk_namespace, &prefix.raw_keys())
    }

    pub fn range_raw<'b>(
        &self,
        store: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Record> + 'b>
    where
        T: 'b,
    {
        self.no_prefix().range_raw(store, min, max, order)
    }

    pub fn keys_raw<'b>(
        &self,
        store: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = Vec<u8>> + 'b>
    where
        T: 'b,
    {
        self.no_prefix().keys_raw(store, min, max, order)
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

    pub fn range<'b>(
        &self,
        storage: &'b dyn Storage,
        min: Option<Bound<K>>,
        max: Option<Bound<K>>,
        order: Order,
    ) -> Box<dyn Iterator<Item = StdResult<(K::Output, T)>> + 'b> {
        self.no_prefix().range(storage, min, max, order)
    }
}

impl<'a, K, T, I, E: Encoding<T>> IndexedMap<'a, K, T, I, E>
where
    K: MapKey,
    E: Encoding<T>,
{
    pub fn load(&self, storage: &dyn Storage, key: K) -> StdResult<T> {
        self.primary.load(storage, key)
    }

    pub fn may_load(&self, storage: &dyn Storage, key: K) -> StdResult<Option<T>> {
        self.primary.may_load(storage, key)
    }
}

impl<'a, K, T, I, E: Encoding<T>> IndexedMap<'a, K, T, I, E>
where
    K: MapKey + Clone,
    I: IndexList<T>,
    E: Encoding<T>,
{
    pub fn save(&'a self, storage: &mut dyn Storage, key: K, data: &T) -> StdResult<()> {
        let old_data = self.may_load(storage, key.clone())?;
        self.replace(storage, key, Some(data), old_data.as_ref())
    }

    pub fn remove(&'a self, storage: &mut dyn Storage, key: K) -> StdResult<()> {
        let old_data = self.may_load(storage, key.clone())?;
        self.replace(storage, key, None, old_data.as_ref())
    }

    pub fn replace(
        &'a self,
        storage: &mut dyn Storage,
        key: K,
        data: Option<&T>,
        old_data: Option<&T>,
    ) -> StdResult<()> {
        // this is the key *relative* to the primary map namespace
        let pk = key.serialize();
        if let Some(old) = old_data {
            for index in self.idx.get_indexes() {
                index.remove(storage, &pk, old)?;
            }
        }
        if let Some(updated) = data {
            for index in self.idx.get_indexes() {
                index.save(storage, &pk, updated)?;
            }
            self.primary.save(storage, key, updated)?;
        } else {
            self.primary.remove(storage, key);
        }
        Ok(())
    }
}

impl<'a, K, T, I, E: Encoding<T>> IndexedMap<'a, K, T, I, E>
where
    K: MapKey + Clone,
    I: IndexList<T>,
    T: Clone,
    E: Encoding<T>,
{
    pub fn update<A, Err>(&'a self, storage: &mut dyn Storage, key: K, action: A) -> Result<T, Err>
    where
        A: FnOnce(Option<T>) -> Result<T, Err>,
        Err: From<StdError>,
    {
        let input = self.may_load(storage, key.clone())?;
        let old_val = input.clone();
        let output = action(input)?;
        self.replace(storage, key, Some(&output), old_val.as_ref())?;
        Ok(output)
    }
}

// ----------------------------------- tests -----------------------------------

#[cfg(test)]
mod tests {

    use {
        crate::{Borsh, Encoding, Index, IndexList, IndexedMap, MultiIndex, Proto, UniqueIndex},
        borsh::{BorshDeserialize, BorshSerialize},
        grug_types::{MockStorage, Order, StdError, StdResult},
        prost::Message,
        std::{str::from_utf8, vec},
        test_case::test_case,
    };

    #[derive(BorshSerialize, BorshDeserialize, PartialEq, Eq, PartialOrd, Ord, Message, Clone)]
    struct Foo {
        #[prost(string, tag = "1")]
        pub name: String,
        #[prost(string, tag = "2")]
        pub surname: String,
        #[prost(uint32, tag = "3")]
        pub id: u32,
    }

    impl Foo {
        pub fn new(name: &str, surname: &str, id: u32) -> Self {
            Foo {
                name: name.to_string(),
                surname: surname.to_string(),
                id,
            }
        }
    }

    struct FooIndexes<'a, E: Encoding<Foo> = Borsh> {
        pub name: MultiIndex<'a, String, Foo, u64, E>,
        pub name_surname: MultiIndex<'a, (String, String), Foo, u64, E>,
        pub id: UniqueIndex<'a, u32, Foo, u64, E>,
    }

    impl<'a, E: Encoding<Foo>> IndexList<Foo> for FooIndexes<'a, E> {
        fn get_indexes(&self) -> Box<dyn Iterator<Item = &'_ dyn Index<Foo>> + '_> {
            let v: Vec<&dyn Index<Foo>> = vec![&self.name, &self.name_surname, &self.id];
            Box::new(v.into_iter())
        }
    }

    fn foo<'a, E: Encoding<Foo>>() -> IndexedMap<'a, u64, Foo, FooIndexes<'a, E>, E> {
        let indexes = FooIndexes::<E> {
            name: MultiIndex::new(|_, data| data.name.clone(), "pk_namespace", "name"),
            name_surname: MultiIndex::new(
                |_, data| (data.name.clone(), data.surname.clone()),
                "pk_namespace",
                "name_surname",
            ),
            id: UniqueIndex::new(|data| data.id, "unique_ns"),
        };

        IndexedMap::<u64, Foo, FooIndexes<'a, E>, E>::new("pk_namespace", indexes)
    }

    fn default<'a, E: Encoding<Foo>>(
    ) -> (MockStorage, IndexedMap<'a, u64, Foo, FooIndexes<'a, E>, E>) {
        let mut deps = MockStorage::new();
        let map: IndexedMap<u64, Foo, FooIndexes<E>, E> = foo::<E>();
        map.save(&mut deps, 1, &Foo::new("bar", "s_bar", 101))
            .unwrap();
        map.save(&mut deps, 2, &Foo::new("bar", "s_bar", 102))
            .unwrap();
        map.save(&mut deps, 3, &Foo::new("bar", "s_foo", 103))
            .unwrap();
        map.save(&mut deps, 4, &Foo::new("foo", "s_foo", 104))
            .unwrap();
        (deps, map)
    }

    fn _keys_to_utf8(bytes: &[u8]) -> StdResult<Vec<String>> {
        let mut bytes = bytes.to_vec();
        let mut res = vec![];

        while !bytes.is_empty() {
            let len = bytes.drain(0..2).collect::<Vec<_>>();
            let len = u16::from_be_bytes(len.try_into().unwrap());
            let decoded_key = if len > bytes.len() as u16 || len == 0 {
                let decoded_key = from_utf8(&bytes)
                    .map_err(|_| StdError::generic_err("Invalid parse to utf8"))?
                    .to_string();
                bytes.clear();
                decoded_key
            } else {
                let (raw_key, rest) = bytes.split_at(len as usize);
                let decoded_key = from_utf8(raw_key)
                    .map_err(|_| StdError::generic_err("Invalid parse to utf8"))?
                    .to_string();
                bytes = rest.to_vec();
                decoded_key
            };
            res.push(decoded_key);
        }
        dbg!(&res);

        Ok(res)
    }

    #[test_case(default::<Proto>(); "proto")]
    #[test_case(default::<Borsh>(); "borsh")]
    fn index_no_prefix<E: Encoding<Foo>>(
        (deps, index): (MockStorage, IndexedMap<u64, Foo, FooIndexes<E>, E>),
    ) {
        let val = index
            .idx
            .name_surname
            .no_prefix()
            .range_raw(&deps, None, None, Order::Ascending)
            .map(|val| val.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            val,
            vec![
                (1_u64.to_be_bytes().to_vec(), Foo::new("bar", "s_bar", 101)),
                (2_u64.to_be_bytes().to_vec(), Foo::new("bar", "s_bar", 102)),
                (3_u64.to_be_bytes().to_vec(), Foo::new("bar", "s_foo", 103)),
                (4_u64.to_be_bytes().to_vec(), Foo::new("foo", "s_foo", 104))
            ]
        );

        let val = index
            .idx
            .name_surname
            .no_prefix()
            .range(&deps, None, None, Order::Ascending)
            .map(|val| val.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            val,
            vec![
                (1, Foo::new("bar", "s_bar", 101)),
                (2, Foo::new("bar", "s_bar", 102)),
                (3, Foo::new("bar", "s_foo", 103)),
                (4, Foo::new("foo", "s_foo", 104))
            ]
        );
    }

    #[test_case(default::<Proto>(); "proto")]
    #[test_case(default::<Borsh>(); "borsh")]
    fn index_prefix<E: Encoding<Foo>>(
        (deps, index): (MockStorage, IndexedMap<u64, Foo, FooIndexes<E>, E>),
    ) {
        let val = index
            .idx
            .name_surname
            .prefix(("bar".to_string(), "s_bar".to_string()))
            .range_raw(&deps, None, None, Order::Ascending)
            .map(|val| val.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            val,
            vec![
                (1_u64.to_be_bytes().to_vec(), Foo::new("bar", "s_bar", 101)),
                (2_u64.to_be_bytes().to_vec(), Foo::new("bar", "s_bar", 102))
            ]
        );

        let val = index
            .idx
            .name_surname
            .prefix(("bar".to_string(), "s_bar".to_string()))
            .range(&deps, None, None, Order::Ascending)
            .map(|val| val.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            val,
            vec![
                (1, Foo::new("bar", "s_bar", 101)),
                (2, Foo::new("bar", "s_bar", 102)),
            ]
        );
    }

    #[test_case(default::<Proto>(); "proto")]
    #[test_case(default::<Borsh>(); "borsh")]
    fn index_sub_prefix<E: Encoding<Foo>>(
        (deps, index): (MockStorage, IndexedMap<u64, Foo, FooIndexes<E>, E>),
    ) {
        let val = index
            .idx
            .name_surname
            .sub_prefix("bar".to_string())
            .range_raw(&deps, None, None, Order::Ascending)
            .map(|val| val.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            val,
            vec![
                (1_u64.to_be_bytes().to_vec(), Foo::new("bar", "s_bar", 101)),
                (2_u64.to_be_bytes().to_vec(), Foo::new("bar", "s_bar", 102)),
                (3_u64.to_be_bytes().to_vec(), Foo::new("bar", "s_foo", 103))
            ]
        );

        let val = index
            .idx
            .name_surname
            .sub_prefix("bar".to_string())
            .range(&deps, None, None, Order::Ascending)
            .map(|val| val.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            val,
            vec![
                (1, Foo::new("bar", "s_bar", 101)),
                (2, Foo::new("bar", "s_bar", 102)),
                (3, Foo::new("bar", "s_foo", 103))
            ]
        );
    }

    #[test_case(default::<Proto>(); "proto")]
    #[test_case(default::<Borsh>(); "borsh")]
    fn unique<E: Encoding<Foo>>(
        (mut deps, index): (MockStorage, IndexedMap<u64, Foo, FooIndexes<E>, E>),
    ) {
        let val = index.idx.id.map().load(&deps, 103).unwrap();
        assert_eq!(val, Foo::new("bar", "s_foo", 103));

        // Try to save a duplicate
        index
            .save(&mut deps, 5, &Foo::new("bar", "s_foo", 103))
            .unwrap_err();

        let val = index
            .idx
            .id
            .map()
            .range(&deps, None, None, Order::Ascending)
            .map(|val| val.unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            val,
            vec![
                (101, Foo::new("bar", "s_bar", 101)),
                (102, Foo::new("bar", "s_bar", 102)),
                (103, Foo::new("bar", "s_foo", 103)),
                (104, Foo::new("foo", "s_foo", 104))
            ]
        );
    }
}