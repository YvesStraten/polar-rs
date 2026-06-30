use crate::error::Error;

pub struct Page<T> {
    pub items: Vec<T>,
    pub max_page: u32,
}

pub fn paginate<T, F, Fut>(mut fetch: F) -> impl futures::Stream<Item = Result<T, Error>>
where
    F: FnMut(u32) -> Fut,
    Fut: std::future::Future<Output = Result<Page<T>, Error>>,
{
    async_stream::try_stream! {
        let mut page: u32 = 1;
        loop {
            let p = fetch(page).await?;
            let max = p.max_page;
            for item in p.items {
                yield item;
            }
            if page >= max {
                break;
            }
            page += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::TryStreamExt;

    #[tokio::test]
    async fn streams_all_items_across_pages() {
        // 3 pages, 2 items each, max_page = 3
        let stream = paginate(|page: u32| async move {
            let base = (page - 1) * 2;
            Ok::<_, Error>(Page {
                items: vec![base, base + 1],
                max_page: 3,
            })
        });
        let all: Vec<u32> = stream.try_collect().await.unwrap();
        assert_eq!(all, vec![0, 1, 2, 3, 4, 5]);
    }

    #[tokio::test]
    async fn stops_after_single_page() {
        let stream = paginate(|_page: u32| async move {
            Ok::<_, Error>(Page {
                items: vec![42],
                max_page: 1,
            })
        });
        let all: Vec<u32> = stream.try_collect().await.unwrap();
        assert_eq!(all, vec![42]);
    }
}
