#[macro_export]
macro_rules! define_rpc {
    ($service:ident { $($function:ident { $($field:ident: $field_type:ty),* } -> $response:ty),* $(,)? }) => {

    paste::paste!{
        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum [<$service Request>] {
            $($function { $($field: $field_type),* }),*
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        pub enum [<$service Response>] {
            $($function($response)),*
        }


        #[allow(async_fn_in_trait)]
        pub trait [<$service Handler>]
        {
            $(
                fn [<$function:snake>](&self, $($field: $field_type),*) -> $response;
            )*

            async fn serve<C>(&mut self, mut channel: C) -> Result<(), $crate::Error>
            where
                C: $crate::Channel<
                    In = [<$service Response>],
                    Out = [<$service Request>],
                >
                {
                    loop {
                        match channel.recv().await? {
                            $([<$service Request>]::$function { $($field),* } => {
                                let response = self.[<$function:snake>]($($field),*);

                                channel.send( [<$service Response>]::$function(response) ).await?;
                            }),*
                        }
                    }
                }
        }


        pub struct $service<C>(C);

        impl<C> $service<C>
        where
            C: $crate::Channel<
                In = [<$service Request>],
                Out = [<$service Response>],
            >
        {
            pub fn bind(channel: C) -> Self {
                Self(channel)
            }

            $(pub async fn [<$function:snake>](
                &mut self, $($field: $field_type),*
            ) -> Result<$response, $crate::Error> {
                let request = [<$service Request>]::$function { $($field),* };

                self.0.send(request).await?;

                match self.0.recv().await? {
                    [<$service Response>]::$function(ret) => Ok(ret),
                    _ => Err($crate::Error::Other {
                        // TODO: we cannot print other here, as that forces Debug
                        // work around by codegen per field? Adds bloat...
                        message: format!("got invalid response for {}", stringify!($function)),
                        source: None
                    }.into()
                    )
                }
            })*
        }
    }};
}
