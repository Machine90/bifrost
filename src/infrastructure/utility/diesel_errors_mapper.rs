use diesel::result::Error;

pub(crate) fn map_diesel_result<T>(result: Result<T, Error>) -> anyhow::Result<Option<T>> {
    let result = match result {
        Ok(result) => Ok(Some(result)),
        Err(Error::NotFound) => Ok(None),
        Err(e) => Err(e)?,
    };
    result
}
