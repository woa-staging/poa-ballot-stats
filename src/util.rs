use colored::{Color, Colorize};
use contracts::v1::voting::logs::BallotCreated as BallotCreatedV1;
use contracts::v2::voting::logs::BallotCreated;
use ethabi::{self, Address, Bytes, FunctionOutputDecoder};
use std::{fmt, u8};
use web3;
use web3::futures::Future;
use web3::helpers::CallFuture;

// TODO: Evaluate whether any of these would make sense to include in `web3`.

/// Executes a function call on the latest block and returns the decoded output.
pub fn raw_call<T: web3::Transport, D: FunctionOutputDecoder>(
    to: Address,
    eth: &web3::api::Eth<T>,
    (bytes, decoder): (Bytes, D),
) -> Result<D::Output, web3::contract::Error> {
    let req = web3::types::CallRequest {
        from: None,
        to,
        gas: None,
        gas_price: None,
        value: None,
        data: Some(bytes.into()),
    };
    let bytes = eth
        .call(req, Some(web3::types::BlockNumber::Latest))
        .wait()?;
    Ok(decoder.decode(&bytes.0)?)
}

trait TopicExt<T> {
    /// Returns the union of the two topics.
    fn or(self, other: Self) -> Self;

    /// Converts this topic into an `Option<Vec<T>>`, where `Any` corresponds to `None`,
    /// `This` to a vector with one element, and `OneOf` to any vector.
    fn to_opt_vec(self) -> Option<Vec<T>>;
}

impl<T: Ord> TopicExt<T> for ethabi::Topic<T> {
    fn or(self, other: Self) -> Self {
        match (self.to_opt_vec(), other.to_opt_vec()) {
            (Some(mut v0), Some(v1)) => {
                for e in v1 {
                    if !v0.contains(&e) {
                        v0.push(e);
                    }
                }
                if v0.len() == 1 {
                    ethabi::Topic::This(v0.into_iter().next().expect("has a single element; qed"))
                } else {
                    ethabi::Topic::OneOf(v0)
                }
            }
            (_, _) => ethabi::Topic::Any,
        }
    }

    fn to_opt_vec(self) -> Option<Vec<T>> {
        match self {
            ethabi::Topic::Any => None,
            ethabi::Topic::OneOf(v) => Some(v),
            ethabi::Topic::This(t) => Some(vec![t]),
        }
    }
}

pub trait TopicFilterExt {
    /// Returns a `web3::types::FilterBuilder` with these topics, starting from the first block.
    fn to_filter_builder(self) -> web3::types::FilterBuilder;

    /// Returns the "disjunction" of the two filters, i.e. it filters for everything that matches
    /// at least one of the two in every topic.
    fn or(self, other: ethabi::TopicFilter) -> ethabi::TopicFilter;

    /// Returns the vector of logs that match this filter.
    fn logs<T: web3::Transport>(
        self,
        web3: &web3::Web3<T>,
    ) -> Result<Vec<web3::types::Log>, web3::error::Error>;
}

impl TopicFilterExt for ethabi::TopicFilter {
    fn to_filter_builder(self) -> web3::types::FilterBuilder {
        web3::types::FilterBuilder::default()
            .topics(
                self.topic0.to_opt_vec(),
                self.topic1.to_opt_vec(),
                self.topic2.to_opt_vec(),
                self.topic3.to_opt_vec(),
            ).from_block(web3::types::BlockNumber::Earliest)
            .to_block(web3::types::BlockNumber::Latest)
    }

    fn or(self, other: ethabi::TopicFilter) -> ethabi::TopicFilter {
        ethabi::TopicFilter {
            topic0: self.topic0.or(other.topic0),
            topic1: self.topic1.or(other.topic1),
            topic2: self.topic2.or(other.topic2),
            topic3: self.topic3.or(other.topic3),
        }
    }

    fn logs<T: web3::Transport>(
        self,
        web3: &web3::Web3<T>,
    ) -> Result<Vec<web3::types::Log>, web3::error::Error> {
        // TODO: Once a version with https://github.com/tomusdrw/rust-web3/pull/122 is available:
        // self.transport.logs(self.to_filter_builder().build())
        let filter = web3::helpers::serialize(&self.to_filter_builder().build());
        CallFuture::new(web3.transport().execute("eth_getLogs", vec![filter])).wait()
    }
}

pub trait Web3LogExt {
    fn into_raw(self) -> ethabi::RawLog;
}

impl Web3LogExt for web3::types::Log {
    fn into_raw(self) -> ethabi::RawLog {
        (self.topics, self.data.0).into()
    }
}

/// Wrapper for a byte array, whose `Display` implementation outputs shortened hexadecimal strings.
pub struct HexBytes<'a>(pub &'a [u8]);

impl<'a> fmt::Display for HexBytes<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x")?;
        for i in &self.0[..2] {
            write!(f, "{:02x}", i)?;
        }
        write!(f, "…")?;
        for i in &self.0[(self.0.len() - 2)..] {
            write!(f, "{:02x}", i)?;
        }
        Ok(())
    }
}

/// Wrapper for a list of byte arrays, whose `Display` implementation outputs shortened hexadecimal
/// strings.
pub struct HexList<'a, T: 'a, I: IntoIterator<Item = &'a T>>(pub I, pub Color);

impl<'a, T: 'a, I: IntoIterator<Item = &'a T> + Clone> fmt::Display for HexList<'a, T, I>
where
    T: AsRef<[u8]>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for (i, item) in self.0.clone().into_iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            let item = format!("{}", HexBytes(item.as_ref()));
            write!(f, "{}", item.color(self.1))?;
        }
        Ok(())
    }
}

/// Conversion into a `BallotCreated` event.
pub trait IntoBallot {
    /// Converts a `BallotCreated` event from earlier contract types into the current one.
    fn into(self) -> BallotCreated;
}

impl IntoBallot for BallotCreatedV1 {
    fn into(self) -> BallotCreated {
        BallotCreated {
            id: self.id,
            ballot_type: self.ballot_type,
            creator: self.creator,
        }
    }
}
