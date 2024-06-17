use {
    crate::{Borsh, Encoding, Proto, RawKey},
    borsh::{BorshDeserialize, BorshSerialize},
    grug_types::{
        from_borsh_slice, from_proto_slice, nested_namespaces_with_key, to_borsh_vec, to_proto_vec,
        StdError, StdResult, Storage,
    },
    prost::Message,
    std::marker::PhantomData,
};

pub struct PathBuf<T, E: Encoding = Borsh> {
    storage_key: Vec<u8>,
    data: PhantomData<T>,
    encoding: PhantomData<E>,
}

impl<T, E> PathBuf<T, E>
where
    E: Encoding,
{
    pub fn new(namespace: &[u8], prefixes: &[RawKey], maybe_key: Option<&RawKey>) -> Self {
        Self {
            storage_key: nested_namespaces_with_key(Some(namespace), prefixes, maybe_key),
            data: PhantomData,
            encoding: PhantomData,
        }
    }

    pub fn as_path(&self) -> Path<'_, T, E> {
        Path {
            storage_key: self.storage_key.as_slice(),
            data: self.data,
            encoding: self.encoding,
        }
    }
}

pub struct Path<'a, T, E: Encoding = Borsh> {
    storage_key: &'a [u8],
    data: PhantomData<T>,
    encoding: PhantomData<E>,
}

impl<'a, T, E> Path<'a, T, E>
where
    E: Encoding,
{
    pub(crate) fn from_raw(storage_key: &'a [u8]) -> Self {
        Self {
            storage_key,
            data: PhantomData,
            encoding: PhantomData,
        }
    }

    pub fn exists(&self, storage: &dyn Storage) -> bool {
        storage.read(self.storage_key).is_some()
    }

    pub fn remove(&self, storage: &mut dyn Storage) {
        storage.remove(self.storage_key);
    }
}

// ----------------------------------- borsh -----------------------------------

impl<'a, T> Path<'a, T, Borsh>
where
    T: BorshSerialize,
{
    pub fn save(&self, storage: &mut dyn Storage, data: &T) -> StdResult<()> {
        let bytes = to_borsh_vec(data)?;
        storage.write(self.storage_key, &bytes);
        Ok(())
    }
}

impl<'a, T> Path<'a, T, Borsh>
where
    T: BorshDeserialize,
{
    pub fn may_load(&self, storage: &dyn Storage) -> StdResult<Option<T>> {
        storage
            .read(self.storage_key)
            .map(from_borsh_slice)
            .transpose()
    }

    pub fn load(&self, storage: &dyn Storage) -> StdResult<T> {
        storage
            .read(self.storage_key)
            .ok_or_else(|| StdError::data_not_found::<T>(self.storage_key))
            .and_then(from_borsh_slice)
    }
}

impl<'a, T> Path<'a, T, Borsh>
where
    T: BorshSerialize + BorshDeserialize,
{
    // compared to the original cosmwasm, we require `action` to return an
    // option, which in case of None leads to the record being deleted.
    pub fn update<A, E>(&self, storage: &mut dyn Storage, action: A) -> Result<Option<T>, E>
    where
        A: FnOnce(Option<T>) -> Result<Option<T>, E>,
        E: From<StdError>,
    {
        let maybe_data = action(self.may_load(storage)?)?;

        if let Some(data) = &maybe_data {
            self.save(storage, data)?;
        } else {
            self.remove(storage);
        }

        Ok(maybe_data)
    }
}

// ----------------------------------- proto -----------------------------------

impl<'a, T> Path<'a, T, Proto>
where
    T: Message,
{
    // Note that for Protobuf, this function doesn't need to return a Result,
    // because proto serialization always succeeds. Anyway a Result is returned
    // to unify the return type among the different implementations.
    pub fn save(&self, storage: &mut dyn Storage, data: &T) -> StdResult<()> {
        let bytes = to_proto_vec(data);
        storage.write(self.storage_key, &bytes);
        Ok(())
    }
}

impl<'a, T> Path<'a, T, Proto>
where
    T: Message + Default,
{
    pub fn may_load(&self, storage: &dyn Storage) -> StdResult<Option<T>> {
        storage
            .read(self.storage_key)
            .map(from_proto_slice)
            .transpose()
    }

    pub fn load(&self, storage: &dyn Storage) -> StdResult<T> {
        storage
            .read(self.storage_key)
            .ok_or_else(|| StdError::data_not_found::<T>(self.storage_key))
            .and_then(from_proto_slice)
    }

    // compared to the original cosmwasm, we require `action` to return an
    // option, which in case of None leads to the record being deleted.
    pub fn update<A, E>(&self, storage: &mut dyn Storage, action: A) -> Result<Option<T>, E>
    where
        A: FnOnce(Option<T>) -> Result<Option<T>, E>,
        E: From<StdError>,
    {
        let maybe_data = action(self.may_load(storage)?)?;

        if let Some(data) = &maybe_data {
            self.save(storage, data)?;
        } else {
            self.remove(storage);
        }

        Ok(maybe_data)
    }
}
