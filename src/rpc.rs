use serde::{Deserialize, Serialize};

#[macro_export]
macro_rules! define_rpc {
    ($service:ident { $($function:ident { $($field:ident: $field_type:ty),* } -> $response:ty),* $(,)? }) => {
    use $crate::{flow, rpc::*};

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

            async fn serve(&mut self, mut flow: impl flow::Flow) -> Result<(), flow::FlowError>
                {
                    loop {
                        match flow.recv().await? {
                            $([<$service Request>]::$function { $($field),* } => {
                                let response = self.[<$function:snake>]($($field),*);

                                flow.send(response).await?;
                            }),*
                        }
                    }
                }
        }


        pub struct $service<F>(F);

        impl<F: flow::Flow> $service<F>
        {
            pub fn bind(flow: F) -> Self {
                Self(flow)
            }

            $(pub async fn [<$function:snake>](
                &mut self, $($field: $field_type),*
            ) -> Result<$response, flow::FlowError> {
                let request = [<$service Request>]::$function { $($field),* };
                self.0.send(request).await?;
                Ok(self.0.recv().await?)
            })*
        }
    }};
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RpcError {
    message: String,
}

impl<T: ToString> From<T> for RpcError {
    fn from(value: T) -> Self {
        Self {
            message: value.to_string(),
        }
    }
}

pub type RpcResult<T> = std::result::Result<T, RpcError>;
