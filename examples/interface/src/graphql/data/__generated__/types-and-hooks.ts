import { gql } from '@apollo/client';
import * as Apollo from '@apollo/client';
export type Maybe<T> = T;
export type InputMaybe<T> = T;
export type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
export type MakeOptional<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]?: Maybe<T[SubKey]> };
export type MakeMaybe<T, K extends keyof T> = Omit<T, K> & { [SubKey in K]: Maybe<T[SubKey]> };
const defaultOptions = {} as const;
/** All built-in and custom scalars, mapped to their actual values */
export type Scalars = {
  ID: string;
  String: string;
  Boolean: boolean;
  Int: number;
  Float: number;
  /**
   * The `AWSJSON` scalar type provided by AWS AppSync, represents a JSON string that
   * complies with [RFC 8259](https://tools.ietf.org/html/rfc8259).  Maps like
   * "**{\\"upvotes\\": 10}**", lists like "**[1,2,3]**", and scalar values like
   * "**\\"AWSJSON example string\\"**", "**1**", and "**true**" are accepted as
   * valid JSON and will automatically be parsed and loaded in the resolver mapping
   * templates as Maps, Lists, or Scalar values rather than as the literal input
   * strings.  Invalid JSON strings like "**{a: 1}**", "**{'a': 1}**" and "**Unquoted
   * string**" will throw GraphQL validation errors.
   */
  AWSJSON: any;
};

/**
 *  Types, unions, and inputs (alphabetized):
 * These are colocated to highlight the relationship between some types and their inputs.
 */
export type ActivityDetails = SwapOrderDetails | TransactionDetails;

export type ActivityDetailsInput = {
  swapOrderDetails?: InputMaybe<SwapOrderDetailsInput>;
  transactionDetails?: InputMaybe<TransactionDetailsInput>;
};

/**
 *  Enums (alphabetized):
 * deprecated and replaced with TransactionType, please do not use this
 */
export enum ActivityType {
  Approve = 'APPROVE',
  Borrow = 'BORROW',
  Burn = 'BURN',
  Cancel = 'CANCEL',
  Claim = 'CLAIM',
  Deployment = 'DEPLOYMENT',
  Lend = 'LEND',
  Mint = 'MINT',
  Nft = 'NFT',
  Receive = 'RECEIVE',
  Repay = 'REPAY',
  Send = 'SEND',
  Stake = 'STAKE',
  Swap = 'SWAP',
  SwapOrder = 'SWAP_ORDER',
  Staking = 'Staking',
  Unknown = 'UNKNOWN',
  Unstake = 'UNSTAKE',
  Withdraw = 'WITHDRAW',
  Market = 'market',
  Money = 'money'
}

export type Amount = IAmount & {
  __typename?: 'Amount';
  currency?: Maybe<Currency>;
  id: Scalars['ID'];
  value: Scalars['Float'];
};

export type AmountChange = {
  __typename?: 'AmountChange';
  absolute?: Maybe<Amount>;
  id: Scalars['ID'];
  percentage?: Maybe<Amount>;
};

export type AmountInput = {
  currency?: InputMaybe<Currency>;
  value: Scalars['Float'];
};

export type ApplicationContract = IContract & {
  __typename?: 'ApplicationContract';
  address: Scalars['String'];
  chain: Chain;
  icon?: Maybe<Image>;
  id: Scalars['ID'];
  name?: Maybe<Scalars['String']>;
};

export type ApplicationContractInput = {
  address: Scalars['String'];
  chain: Chain;
  icon?: InputMaybe<ImageInput>;
  name?: InputMaybe<Scalars['String']>;
};

export type AssetActivity = {
  __typename?: 'AssetActivity';
  addresses?: Maybe<Array<Scalars['String']>>;
  /** @deprecated use assetChanges field in details */
  assetChanges: Array<Maybe<AssetChange>>;
  chain: Chain;
  details: ActivityDetails;
  /** @deprecated not required, remove usage */
  gasUsed?: Maybe<Scalars['Float']>;
  id: Scalars['ID'];
  timestamp: Scalars['Int'];
  /** @deprecated use fields from details */
  transaction: Transaction;
  /** @deprecated use type field in details */
  type: ActivityType;
};

export type AssetActivityInput = {
  chain: Chain;
  details: ActivityDetailsInput;
  timestamp: Scalars['Int'];
};

export enum AssetActivitySwitch {
  Alternate = 'ALTERNATE',
  Legacy = 'LEGACY'
}

export type AssetChange = NftApproval | NftApproveForAll | NftTransfer | TokenApproval | TokenTransfer;

export type AssetChangeInput = {
  nftApproval?: InputMaybe<NftApprovalInput>;
  nftApproveForAll?: InputMaybe<NftApproveForAllInput>;
  nftTransfer?: InputMaybe<NftTransferInput>;
  tokenApproval?: InputMaybe<TokenApprovalInput>;
  tokenTransfer?: InputMaybe<TokenTransferInput>;
};

export enum Chain {
  Arbitrum = 'ARBITRUM',
  Avalanche = 'AVALANCHE',
  Base = 'BASE',
  Blast = 'BLAST',
  Bnb = 'BNB',
  Celo = 'CELO',
  Ethereum = 'ETHEREUM',
  EthereumGoerli = 'ETHEREUM_GOERLI',
  EthereumSepolia = 'ETHEREUM_SEPOLIA',
  Optimism = 'OPTIMISM',
  Polygon = 'POLYGON',
  UnknownChain = 'UNKNOWN_CHAIN'
}

export enum CollectionSortableField {
  Volume = 'VOLUME'
}

export type ContractInput = {
  address?: InputMaybe<Scalars['String']>;
  chain: Chain;
};

export enum Currency {
  Aud = 'AUD',
  Brl = 'BRL',
  Cad = 'CAD',
  Cny = 'CNY',
  Eth = 'ETH',
  Eur = 'EUR',
  Gbp = 'GBP',
  Hkd = 'HKD',
  Idr = 'IDR',
  Inr = 'INR',
  Jpy = 'JPY',
  Matic = 'MATIC',
  Ngn = 'NGN',
  Pkr = 'PKR',
  Rub = 'RUB',
  Sgd = 'SGD',
  Thb = 'THB',
  Try = 'TRY',
  Uah = 'UAH',
  Usd = 'USD',
  Vnd = 'VND'
}

export type CurrencyAmountInput = {
  currency: Currency;
  value: Scalars['Float'];
};

export type DescriptionTranslations = {
  __typename?: 'DescriptionTranslations';
  descriptionEnUs?: Maybe<Scalars['String']>;
  descriptionEs419?: Maybe<Scalars['String']>;
  descriptionEsEs?: Maybe<Scalars['String']>;
  descriptionEsUs?: Maybe<Scalars['String']>;
  descriptionFrFr?: Maybe<Scalars['String']>;
  descriptionHiIn?: Maybe<Scalars['String']>;
  descriptionIdId?: Maybe<Scalars['String']>;
  descriptionJaJp?: Maybe<Scalars['String']>;
  descriptionMsMy?: Maybe<Scalars['String']>;
  descriptionNlNl?: Maybe<Scalars['String']>;
  descriptionPtPt?: Maybe<Scalars['String']>;
  descriptionRuRu?: Maybe<Scalars['String']>;
  descriptionThTh?: Maybe<Scalars['String']>;
  descriptionTrTr?: Maybe<Scalars['String']>;
  descriptionUkUa?: Maybe<Scalars['String']>;
  descriptionUrPk?: Maybe<Scalars['String']>;
  descriptionViVn?: Maybe<Scalars['String']>;
  descriptionZhHans?: Maybe<Scalars['String']>;
  descriptionZhHant?: Maybe<Scalars['String']>;
  id: Scalars['ID'];
};

export type Dimensions = {
  __typename?: 'Dimensions';
  height?: Maybe<Scalars['Float']>;
  id: Scalars['ID'];
  width?: Maybe<Scalars['Float']>;
};

export type DimensionsInput = {
  height?: InputMaybe<Scalars['Float']>;
  width?: InputMaybe<Scalars['Float']>;
};

export type FeeData = {
  __typename?: 'FeeData';
  buyFeeBps?: Maybe<Scalars['String']>;
  sellFeeBps?: Maybe<Scalars['String']>;
};

export enum HighLow {
  High = 'HIGH',
  Low = 'LOW'
}

/**   FIVE_MINUTE is only supported for TokenMarket.pricePercentChange */
export enum HistoryDuration {
  Day = 'DAY',
  FiveMinute = 'FIVE_MINUTE',
  Hour = 'HOUR',
  Max = 'MAX',
  Month = 'MONTH',
  Week = 'WEEK',
  Year = 'YEAR'
}

/**   Interfaces (alphabetized): */
export type IAmount = {
  currency?: Maybe<Currency>;
  value: Scalars['Float'];
};

export type IContract = {
  address?: Maybe<Scalars['String']>;
  chain: Chain;
};

export type IPool = {
  address: Scalars['String'];
  chain: Chain;
  createdAtTimestamp?: Maybe<Scalars['Int']>;
  cumulativeVolume?: Maybe<Amount>;
  historicalVolume?: Maybe<Array<Maybe<TimestampedAmount>>>;
  id: Scalars['ID'];
  priceHistory?: Maybe<Array<Maybe<TimestampedPoolPrice>>>;
  protocolVersion: ProtocolVersion;
  token0?: Maybe<Token>;
  token0Supply?: Maybe<Scalars['Float']>;
  token1?: Maybe<Token>;
  token1Supply?: Maybe<Scalars['Float']>;
  totalLiquidity?: Maybe<Amount>;
  totalLiquidityPercentChange24h?: Maybe<Amount>;
  transactions?: Maybe<Array<Maybe<PoolTransaction>>>;
  txCount?: Maybe<Scalars['Int']>;
};


export type IPoolCumulativeVolumeArgs = {
  duration: HistoryDuration;
};


export type IPoolHistoricalVolumeArgs = {
  duration: HistoryDuration;
};


export type IPoolPriceHistoryArgs = {
  duration: HistoryDuration;
};


export type IPoolTransactionsArgs = {
  first: Scalars['Int'];
  timestampCursor?: InputMaybe<Scalars['Int']>;
};

export type Image = {
  __typename?: 'Image';
  dimensions?: Maybe<Dimensions>;
  id: Scalars['ID'];
  url: Scalars['String'];
};

export type ImageInput = {
  dimensions?: InputMaybe<DimensionsInput>;
  url: Scalars['String'];
};

export enum MediaType {
  Audio = 'AUDIO',
  Image = 'IMAGE',
  Raw = 'RAW',
  Video = 'VIDEO'
}

export type Mutation = {
  __typename?: 'Mutation';
  assetActivity: AssetActivity;
  heartbeat: Status;
  unsubscribe: Status;
};


export type MutationAssetActivityArgs = {
  input: AssetActivityInput;
};


export type MutationHeartbeatArgs = {
  subscriptionId: Scalars['ID'];
  type: SubscriptionType;
};


export type MutationUnsubscribeArgs = {
  subscriptionId: Scalars['ID'];
  type: SubscriptionType;
};

export type NftActivity = {
  __typename?: 'NftActivity';
  address: Scalars['String'];
  asset?: Maybe<NftAsset>;
  fromAddress: Scalars['String'];
  id: Scalars['ID'];
  marketplace?: Maybe<Scalars['String']>;
  orderStatus?: Maybe<OrderStatus>;
  price?: Maybe<Amount>;
  quantity?: Maybe<Scalars['Int']>;
  timestamp: Scalars['Int'];
  toAddress?: Maybe<Scalars['String']>;
  tokenId?: Maybe<Scalars['String']>;
  transactionHash?: Maybe<Scalars['String']>;
  type: NftActivityType;
  url?: Maybe<Scalars['String']>;
};

export type NftActivityConnection = {
  __typename?: 'NftActivityConnection';
  edges: Array<NftActivityEdge>;
  pageInfo: PageInfo;
};

export type NftActivityEdge = {
  __typename?: 'NftActivityEdge';
  cursor: Scalars['String'];
  node: NftActivity;
};

export type NftActivityFilterInput = {
  activityTypes?: InputMaybe<Array<NftActivityType>>;
  address?: InputMaybe<Scalars['String']>;
  tokenId?: InputMaybe<Scalars['String']>;
};

export enum NftActivityType {
  CancelListing = 'CANCEL_LISTING',
  Listing = 'LISTING',
  Sale = 'SALE',
  Transfer = 'TRANSFER'
}

export type NftApproval = {
  __typename?: 'NftApproval';
  approvedAddress: Scalars['String'];
  /**   can be erc721, erc1155, noncompliant */
  asset: NftAsset;
  id: Scalars['ID'];
  nftStandard: NftStandard;
};

export type NftApprovalInput = {
  approvedAddress: Scalars['String'];
  asset: NftAssetInput;
  nftStandard: NftStandard;
};

export type NftApproveForAll = {
  __typename?: 'NftApproveForAll';
  approved: Scalars['Boolean'];
  /**   can be erc721, erc1155, noncompliant */
  asset: NftAsset;
  id: Scalars['ID'];
  nftStandard: NftStandard;
  operatorAddress: Scalars['String'];
};

export type NftApproveForAllInput = {
  approved: Scalars['Boolean'];
  asset: NftAssetInput;
  nftStandard: NftStandard;
  operatorAddress: Scalars['String'];
};

export type NftAsset = {
  __typename?: 'NftAsset';
  animationUrl?: Maybe<Scalars['String']>;
  collection?: Maybe<NftCollection>;
  creator?: Maybe<NftProfile>;
  description?: Maybe<Scalars['String']>;
  flaggedBy?: Maybe<Scalars['String']>;
  id: Scalars['ID'];
  image?: Maybe<Image>;
  /** @deprecated Field no longer supported */
  imageUrl?: Maybe<Scalars['String']>;
  isSpam?: Maybe<Scalars['Boolean']>;
  listings?: Maybe<NftOrderConnection>;
  mediaType?: Maybe<MediaType>;
  metadataUrl?: Maybe<Scalars['String']>;
  name?: Maybe<Scalars['String']>;
  nftContract?: Maybe<NftContract>;
  originalImage?: Maybe<Image>;
  /**   TODO: may need to be array to support erc1155 cases. not needed at the moment so will revisit. */
  ownerAddress?: Maybe<Scalars['String']>;
  rarities?: Maybe<Array<NftAssetRarity>>;
  smallImage?: Maybe<Image>;
  /** @deprecated Field no longer supported */
  smallImageUrl?: Maybe<Scalars['String']>;
  suspiciousFlag?: Maybe<Scalars['Boolean']>;
  thumbnail?: Maybe<Image>;
  /** @deprecated Field no longer supported */
  thumbnailUrl?: Maybe<Scalars['String']>;
  tokenId: Scalars['String'];
  traits?: Maybe<Array<NftAssetTrait>>;
};


export type NftAssetListingsArgs = {
  after?: InputMaybe<Scalars['String']>;
  asc?: InputMaybe<Scalars['Boolean']>;
  before?: InputMaybe<Scalars['String']>;
  chain?: InputMaybe<Chain>;
  first?: InputMaybe<Scalars['Int']>;
  last?: InputMaybe<Scalars['Int']>;
};

export type NftAssetConnection = {
  __typename?: 'NftAssetConnection';
  edges: Array<NftAssetEdge>;
  pageInfo: PageInfo;
  totalCount?: Maybe<Scalars['Int']>;
};

export type NftAssetEdge = {
  __typename?: 'NftAssetEdge';
  cursor: Scalars['String'];
  node: NftAsset;
};

export type NftAssetInput = {
  animationUrl?: InputMaybe<Scalars['String']>;
  collection?: InputMaybe<NftCollectionInput>;
  description?: InputMaybe<Scalars['String']>;
  image?: InputMaybe<ImageInput>;
  isSpam?: InputMaybe<Scalars['Boolean']>;
  mediaType?: InputMaybe<MediaType>;
  name?: InputMaybe<Scalars['String']>;
  nftContract?: InputMaybe<NftContractInput>;
  smallImage?: InputMaybe<ImageInput>;
  thumbnail?: InputMaybe<ImageInput>;
  tokenId: Scalars['String'];
};

export type NftAssetRarity = {
  __typename?: 'NftAssetRarity';
  id: Scalars['ID'];
  provider?: Maybe<NftRarityProvider>;
  rank?: Maybe<Scalars['Int']>;
  score?: Maybe<Scalars['Float']>;
};

export enum NftAssetSortableField {
  Price = 'PRICE',
  Rarity = 'RARITY'
}

export type NftAssetTrait = {
  __typename?: 'NftAssetTrait';
  id: Scalars['ID'];
  name?: Maybe<Scalars['String']>;
  rarity?: Maybe<Scalars['Float']>;
  value?: Maybe<Scalars['String']>;
};

export type NftAssetTraitInput = {
  name: Scalars['String'];
  values: Array<Scalars['String']>;
};

export type NftAssetsFilterInput = {
  listed?: InputMaybe<Scalars['Boolean']>;
  marketplaces?: InputMaybe<Array<NftMarketplace>>;
  maxPrice?: InputMaybe<Scalars['String']>;
  minPrice?: InputMaybe<Scalars['String']>;
  tokenIds?: InputMaybe<Array<Scalars['String']>>;
  tokenSearchQuery?: InputMaybe<Scalars['String']>;
  traits?: InputMaybe<Array<NftAssetTraitInput>>;
};

export type NftBalance = {
  __typename?: 'NftBalance';
  id: Scalars['ID'];
  lastPrice?: Maybe<TimestampedAmount>;
  listedMarketplaces?: Maybe<Array<NftMarketplace>>;
  listingFees?: Maybe<Array<Maybe<NftFee>>>;
  ownedAsset?: Maybe<NftAsset>;
  quantity?: Maybe<Scalars['Int']>;
};

export type NftBalanceAssetInput = {
  address: Scalars['String'];
  tokenId: Scalars['String'];
};

export type NftBalanceConnection = {
  __typename?: 'NftBalanceConnection';
  edges: Array<NftBalanceEdge>;
  pageInfo: PageInfo;
};

export type NftBalanceEdge = {
  __typename?: 'NftBalanceEdge';
  cursor: Scalars['String'];
  node: NftBalance;
};

export type NftBalancesFilterInput = {
  addresses?: InputMaybe<Array<Scalars['String']>>;
  assets?: InputMaybe<Array<NftBalanceAssetInput>>;
  filterSpam?: InputMaybe<Scalars['Boolean']>;
};

export type NftCollection = {
  __typename?: 'NftCollection';
  bannerImage?: Maybe<Image>;
  /**
   *  TODO: support querying for collection assets here
   * assets(page: Int, pageSize: Int, orderBy: NftAssetSortableField): [NftAsset]
   * @deprecated Field no longer supported
   */
  bannerImageUrl?: Maybe<Scalars['String']>;
  collectionId: Scalars['String'];
  creator?: Maybe<NftProfile>;
  description?: Maybe<Scalars['String']>;
  discordUrl?: Maybe<Scalars['String']>;
  homepageUrl?: Maybe<Scalars['String']>;
  id: Scalars['ID'];
  image?: Maybe<Image>;
  /** @deprecated Field no longer supported */
  imageUrl?: Maybe<Scalars['String']>;
  instagramName?: Maybe<Scalars['String']>;
  isVerified?: Maybe<Scalars['Boolean']>;
  markets?: Maybe<Array<NftCollectionMarket>>;
  name?: Maybe<Scalars['String']>;
  nftContracts?: Maybe<Array<NftContract>>;
  numAssets?: Maybe<Scalars['Int']>;
  /** @deprecated Field no longer supported */
  openseaUrl?: Maybe<Scalars['String']>;
  traits?: Maybe<Array<NftCollectionTrait>>;
  twitterName?: Maybe<Scalars['String']>;
};


export type NftCollectionMarketsArgs = {
  currencies: Array<Currency>;
};

export type NftCollectionBalance = {
  __typename?: 'NftCollectionBalance';
  address: Scalars['String'];
  balance: Scalars['Float'];
  id: Scalars['ID'];
  logoImage?: Maybe<Image>;
  name: Scalars['String'];
};

export type NftCollectionBalanceConnection = {
  __typename?: 'NftCollectionBalanceConnection';
  edges: Array<NftCollectionBalanceEdge>;
  pageInfo: PageInfo;
};

export type NftCollectionBalanceEdge = {
  __typename?: 'NftCollectionBalanceEdge';
  cursor: Scalars['String'];
  node: NftCollectionBalance;
};

export type NftCollectionConnection = {
  __typename?: 'NftCollectionConnection';
  edges: Array<NftCollectionEdge>;
  pageInfo: PageInfo;
};

export type NftCollectionEdge = {
  __typename?: 'NftCollectionEdge';
  cursor: Scalars['String'];
  node: NftCollection;
};

export type NftCollectionInput = {
  collectionId: Scalars['String'];
  name?: InputMaybe<Scalars['String']>;
  nftContracts?: InputMaybe<Array<NftContractInput>>;
};

export type NftCollectionMarket = {
  __typename?: 'NftCollectionMarket';
  floorPrice?: Maybe<TimestampedAmount>;
  floorPricePercentChange?: Maybe<TimestampedAmount>;
  id: Scalars['ID'];
  listings?: Maybe<TimestampedAmount>;
  marketplaces?: Maybe<Array<NftCollectionMarketplace>>;
  nftContracts?: Maybe<Array<NftContract>>;
  owners?: Maybe<Scalars['Int']>;
  percentListed?: Maybe<TimestampedAmount>;
  percentUniqueOwners?: Maybe<TimestampedAmount>;
  sales?: Maybe<TimestampedAmount>;
  totalVolume?: Maybe<TimestampedAmount>;
  volume?: Maybe<TimestampedAmount>;
  /** @deprecated Field no longer supported */
  volume24h?: Maybe<Amount>;
  volumePercentChange?: Maybe<TimestampedAmount>;
};


export type NftCollectionMarketFloorPricePercentChangeArgs = {
  duration?: InputMaybe<HistoryDuration>;
};


export type NftCollectionMarketMarketplacesArgs = {
  marketplaces?: InputMaybe<Array<NftMarketplace>>;
};


export type NftCollectionMarketSalesArgs = {
  duration?: InputMaybe<HistoryDuration>;
};


export type NftCollectionMarketVolumeArgs = {
  duration?: InputMaybe<HistoryDuration>;
};


export type NftCollectionMarketVolumePercentChangeArgs = {
  duration?: InputMaybe<HistoryDuration>;
};

export type NftCollectionMarketplace = {
  __typename?: 'NftCollectionMarketplace';
  floorPrice?: Maybe<Scalars['Float']>;
  id: Scalars['ID'];
  listings?: Maybe<Scalars['Int']>;
  marketplace?: Maybe<NftMarketplace>;
};

export type NftCollectionTrait = {
  __typename?: 'NftCollectionTrait';
  id: Scalars['ID'];
  name?: Maybe<Scalars['String']>;
  stats?: Maybe<Array<NftCollectionTraitStats>>;
  values?: Maybe<Array<Scalars['String']>>;
};

export type NftCollectionTraitStats = {
  __typename?: 'NftCollectionTraitStats';
  assets?: Maybe<Scalars['Int']>;
  id: Scalars['ID'];
  listings?: Maybe<Scalars['Int']>;
  name?: Maybe<Scalars['String']>;
  value?: Maybe<Scalars['String']>;
};

export type NftCollectionsFilterInput = {
  addresses?: InputMaybe<Array<Scalars['String']>>;
  nameQuery?: InputMaybe<Scalars['String']>;
};

export type NftContract = IContract & {
  __typename?: 'NftContract';
  address: Scalars['String'];
  chain: Chain;
  id: Scalars['ID'];
  name?: Maybe<Scalars['String']>;
  standard?: Maybe<NftStandard>;
  symbol?: Maybe<Scalars['String']>;
  totalSupply?: Maybe<Scalars['Int']>;
};

export type NftContractInput = {
  address: Scalars['String'];
  chain: Chain;
  name?: InputMaybe<Scalars['String']>;
  standard?: InputMaybe<NftStandard>;
  symbol?: InputMaybe<Scalars['String']>;
  totalSupply?: InputMaybe<Scalars['Int']>;
};

export type NftFee = {
  __typename?: 'NftFee';
  basisPoints: Scalars['Int'];
  id: Scalars['ID'];
  payoutAddress: Scalars['String'];
};

export enum NftMarketplace {
  Cryptopunks = 'CRYPTOPUNKS',
  Foundation = 'FOUNDATION',
  Looksrare = 'LOOKSRARE',
  Nft20 = 'NFT20',
  Nftx = 'NFTX',
  Opensea = 'OPENSEA',
  Sudoswap = 'SUDOSWAP',
  X2Y2 = 'X2Y2'
}

export type NftOrder = {
  __typename?: 'NftOrder';
  address: Scalars['String'];
  auctionType?: Maybe<Scalars['String']>;
  createdAt: Scalars['Float'];
  endAt?: Maybe<Scalars['Float']>;
  id: Scalars['ID'];
  maker: Scalars['String'];
  marketplace: NftMarketplace;
  marketplaceUrl: Scalars['String'];
  orderHash?: Maybe<Scalars['String']>;
  poolPrices?: Maybe<Array<Scalars['String']>>;
  price: Amount;
  protocolParameters?: Maybe<Scalars['AWSJSON']>;
  quantity: Scalars['Int'];
  startAt: Scalars['Float'];
  status: OrderStatus;
  taker?: Maybe<Scalars['String']>;
  tokenId?: Maybe<Scalars['String']>;
  type: OrderType;
};

export type NftOrderConnection = {
  __typename?: 'NftOrderConnection';
  edges: Array<NftOrderEdge>;
  pageInfo: PageInfo;
};

export type NftOrderEdge = {
  __typename?: 'NftOrderEdge';
  cursor: Scalars['String'];
  node: NftOrder;
};

export type NftProfile = {
  __typename?: 'NftProfile';
  address: Scalars['String'];
  id: Scalars['ID'];
  isVerified?: Maybe<Scalars['Boolean']>;
  profileImage?: Maybe<Image>;
  username?: Maybe<Scalars['String']>;
};

export enum NftRarityProvider {
  RaritySniper = 'RARITY_SNIPER'
}

export type NftRouteResponse = {
  __typename?: 'NftRouteResponse';
  calldata: Scalars['String'];
  id: Scalars['ID'];
  route?: Maybe<Array<NftTrade>>;
  sendAmount: TokenAmount;
  toAddress: Scalars['String'];
};

export enum NftStandard {
  Erc721 = 'ERC721',
  Erc1155 = 'ERC1155',
  Noncompliant = 'NONCOMPLIANT'
}

export type NftTrade = {
  __typename?: 'NftTrade';
  amount: Scalars['Int'];
  contractAddress: Scalars['String'];
  id: Scalars['ID'];
  marketplace: NftMarketplace;
  /**   price represents the current price of the NFT, which can be different from quotePrice */
  price: TokenAmount;
  /**   quotePrice represents the last quoted price of the NFT */
  quotePrice?: Maybe<TokenAmount>;
  tokenId: Scalars['String'];
  tokenType?: Maybe<NftStandard>;
};

export type NftTradeInput = {
  amount: Scalars['Int'];
  contractAddress: Scalars['String'];
  id: Scalars['ID'];
  marketplace: NftMarketplace;
  quotePrice?: InputMaybe<TokenAmountInput>;
  tokenId: Scalars['String'];
  tokenType?: InputMaybe<NftStandard>;
};

export type NftTransfer = {
  __typename?: 'NftTransfer';
  asset: NftAsset;
  direction: TransactionDirection;
  id: Scalars['ID'];
  nftStandard: NftStandard;
  recipient: Scalars['String'];
  sender: Scalars['String'];
};

export type NftTransferInput = {
  asset: NftAssetInput;
  direction: TransactionDirection;
  nftStandard: NftStandard;
  recipient: Scalars['String'];
  sender: Scalars['String'];
};

export enum OrderStatus {
  Cancelled = 'CANCELLED',
  Executed = 'EXECUTED',
  Expired = 'EXPIRED',
  Valid = 'VALID'
}

export enum OrderType {
  Listing = 'LISTING',
  Offer = 'OFFER'
}

export type PageInfo = {
  __typename?: 'PageInfo';
  endCursor?: Maybe<Scalars['String']>;
  hasNextPage?: Maybe<Scalars['Boolean']>;
  hasPreviousPage?: Maybe<Scalars['Boolean']>;
  startCursor?: Maybe<Scalars['String']>;
};

/**   v2 pool parameters as defined by https://github.com/Uniswap/v2-sdk/blob/main/src/entities/pair.ts */
export type PairInput = {
  tokenAmountA: TokenAmountInput;
  tokenAmountB: TokenAmountInput;
};

export type PermitDetailsInput = {
  amount: Scalars['String'];
  expiration: Scalars['String'];
  nonce: Scalars['String'];
  token: Scalars['String'];
};

export type PermitInput = {
  details: PermitDetailsInput;
  sigDeadline: Scalars['String'];
  signature: Scalars['String'];
  spender: Scalars['String'];
};

/**   v3 pool parameters as defined by https://github.com/Uniswap/v3-sdk/blob/main/src/entities/pool.ts */
export type PoolInput = {
  fee: Scalars['Int'];
  liquidity: Scalars['String'];
  sqrtRatioX96: Scalars['String'];
  tickCurrent: Scalars['String'];
  tokenA: TokenInput;
  tokenB: TokenInput;
};

export type PoolTransaction = {
  __typename?: 'PoolTransaction';
  account: Scalars['String'];
  chain: Chain;
  hash: Scalars['String'];
  id: Scalars['ID'];
  protocolVersion: ProtocolVersion;
  timestamp: Scalars['Int'];
  token0: Token;
  token0Quantity: Scalars['String'];
  token1: Token;
  token1Quantity: Scalars['String'];
  type: PoolTransactionType;
  usdValue: Amount;
};

export enum PoolTransactionType {
  Add = 'ADD',
  Remove = 'REMOVE',
  Swap = 'SWAP'
}

export type Portfolio = {
  __typename?: 'Portfolio';
  assetActivities?: Maybe<Array<Maybe<AssetActivity>>>;
  id: Scalars['ID'];
  /**   TODO: (michael.zhang) replace with paginated query */
  nftBalances?: Maybe<Array<Maybe<NftBalance>>>;
  ownerAddress: Scalars['String'];
  tokenBalances?: Maybe<Array<Maybe<TokenBalance>>>;
  tokensTotalDenominatedValue?: Maybe<Amount>;
  tokensTotalDenominatedValueChange?: Maybe<AmountChange>;
};


export type PortfolioAssetActivitiesArgs = {
  _fs?: InputMaybe<AssetActivitySwitch>;
  chains?: InputMaybe<Array<Chain>>;
  includeOffChain?: InputMaybe<Scalars['Boolean']>;
  page?: InputMaybe<Scalars['Int']>;
  pageSize?: InputMaybe<Scalars['Int']>;
};


export type PortfolioTokensTotalDenominatedValueChangeArgs = {
  duration?: InputMaybe<HistoryDuration>;
};

/**   Specify how the portfolio value should be calculated for each `ownerAddress`. */
export type PortfolioValueModifier = {
  includeSmallBalances?: InputMaybe<Scalars['Boolean']>;
  includeSpamTokens?: InputMaybe<Scalars['Boolean']>;
  ownerAddress: Scalars['String'];
  tokenExcludeOverrides?: InputMaybe<Array<ContractInput>>;
  tokenIncludeOverrides?: InputMaybe<Array<ContractInput>>;
};

export enum PriceSource {
  SubgraphV2 = 'SUBGRAPH_V2',
  SubgraphV3 = 'SUBGRAPH_V3'
}

export enum ProtocolVersion {
  V2 = 'V2',
  V3 = 'V3'
}

export type PushNotification = {
  __typename?: 'PushNotification';
  contents: Scalars['AWSJSON'];
  id: Scalars['ID'];
  notifyAddress: Scalars['String'];
  signerHeader: Scalars['AWSJSON'];
  viewerHeader: Scalars['AWSJSON'];
};

export type Query = {
  __typename?: 'Query';
  convert?: Maybe<Amount>;
  dailyProtocolTvl?: Maybe<Array<TimestampedAmount>>;
  historicalProtocolVolume?: Maybe<Array<TimestampedAmount>>;
  nftActivity?: Maybe<NftActivityConnection>;
  nftAssets?: Maybe<NftAssetConnection>;
  nftBalances?: Maybe<NftBalanceConnection>;
  nftCollectionBalances?: Maybe<NftCollectionBalanceConnection>;
  nftCollections?: Maybe<NftCollectionConnection>;
  nftRoute?: Maybe<NftRouteResponse>;
  portfolios?: Maybe<Array<Maybe<Portfolio>>>;
  searchTokens?: Maybe<Array<Maybe<Token>>>;
  /**
   *  token consumes chain and address instead of contract because the apollo client request cache can only use
   * keys from the response, and the token response does not contain a contract, but does contain an unwrapped
   * contract: chain and address.
   */
  token?: Maybe<Token>;
  tokenProjects?: Maybe<Array<Maybe<TokenProject>>>;
  tokens?: Maybe<Array<Maybe<Token>>>;
  topCollections?: Maybe<NftCollectionConnection>;
  topTokens?: Maybe<Array<Maybe<Token>>>;
  /**   returns top v2 pairs sorted by total value locked in desc order */
  topV2Pairs?: Maybe<Array<V2Pair>>;
  /**   returns top v3 pools sorted by total value locked in desc order */
  topV3Pools?: Maybe<Array<V3Pool>>;
  transactionNotification?: Maybe<TransactionNotification>;
  v2Pair?: Maybe<V2Pair>;
  v2Transactions?: Maybe<Array<Maybe<PoolTransaction>>>;
  v3Pool?: Maybe<V3Pool>;
  v3Transactions?: Maybe<Array<PoolTransaction>>;
};


export type QueryConvertArgs = {
  fromAmount: CurrencyAmountInput;
  toCurrency: Currency;
};


export type QueryDailyProtocolTvlArgs = {
  chain: Chain;
  version: ProtocolVersion;
};


export type QueryHistoricalProtocolVolumeArgs = {
  chain: Chain;
  duration: HistoryDuration;
  version: ProtocolVersion;
};


export type QueryNftActivityArgs = {
  after?: InputMaybe<Scalars['String']>;
  chain?: InputMaybe<Chain>;
  filter?: InputMaybe<NftActivityFilterInput>;
  first?: InputMaybe<Scalars['Int']>;
};


export type QueryNftAssetsArgs = {
  address: Scalars['String'];
  after?: InputMaybe<Scalars['String']>;
  asc?: InputMaybe<Scalars['Boolean']>;
  before?: InputMaybe<Scalars['String']>;
  chain?: InputMaybe<Chain>;
  filter?: InputMaybe<NftAssetsFilterInput>;
  first?: InputMaybe<Scalars['Int']>;
  last?: InputMaybe<Scalars['Int']>;
  orderBy?: InputMaybe<NftAssetSortableField>;
};


export type QueryNftBalancesArgs = {
  after?: InputMaybe<Scalars['String']>;
  before?: InputMaybe<Scalars['String']>;
  chain?: InputMaybe<Chain>;
  filter?: InputMaybe<NftBalancesFilterInput>;
  first?: InputMaybe<Scalars['Int']>;
  last?: InputMaybe<Scalars['Int']>;
  ownerAddress: Scalars['String'];
};


export type QueryNftCollectionBalancesArgs = {
  after?: InputMaybe<Scalars['String']>;
  before?: InputMaybe<Scalars['String']>;
  chain?: InputMaybe<Chain>;
  first?: InputMaybe<Scalars['Int']>;
  last?: InputMaybe<Scalars['Int']>;
  ownerAddress: Scalars['String'];
};


export type QueryNftCollectionsArgs = {
  after?: InputMaybe<Scalars['String']>;
  chain?: InputMaybe<Chain>;
  filter?: InputMaybe<NftCollectionsFilterInput>;
  first?: InputMaybe<Scalars['Int']>;
};


export type QueryNftRouteArgs = {
  chain?: InputMaybe<Chain>;
  nftTrades: Array<NftTradeInput>;
  senderAddress: Scalars['String'];
  tokenTrades?: InputMaybe<Array<TokenTradeInput>>;
};


export type QueryPortfoliosArgs = {
  chains?: InputMaybe<Array<Chain>>;
  lookupTokens?: InputMaybe<Array<ContractInput>>;
  ownerAddresses: Array<Scalars['String']>;
  valueModifiers?: InputMaybe<Array<PortfolioValueModifier>>;
};


export type QuerySearchTokensArgs = {
  chains?: InputMaybe<Array<Chain>>;
  searchQuery: Scalars['String'];
};


export type QueryTokenArgs = {
  address?: InputMaybe<Scalars['String']>;
  chain: Chain;
};


export type QueryTokenProjectsArgs = {
  contracts: Array<ContractInput>;
};


export type QueryTokensArgs = {
  contracts: Array<ContractInput>;
};


export type QueryTopCollectionsArgs = {
  after?: InputMaybe<Scalars['String']>;
  chains?: InputMaybe<Array<Chain>>;
  cursor?: InputMaybe<Scalars['String']>;
  duration?: InputMaybe<HistoryDuration>;
  first?: InputMaybe<Scalars['Int']>;
  limit?: InputMaybe<Scalars['Int']>;
  orderBy?: InputMaybe<CollectionSortableField>;
};


export type QueryTopTokensArgs = {
  chain?: InputMaybe<Chain>;
  orderBy?: InputMaybe<TokenSortableField>;
  page?: InputMaybe<Scalars['Int']>;
  pageSize?: InputMaybe<Scalars['Int']>;
};


export type QueryTopV2PairsArgs = {
  chain: Chain;
  first: Scalars['Int'];
  tokenFilter?: InputMaybe<Scalars['String']>;
  tvlCursor?: InputMaybe<Scalars['Float']>;
};


export type QueryTopV3PoolsArgs = {
  chain: Chain;
  first: Scalars['Int'];
  tokenFilter?: InputMaybe<Scalars['String']>;
  tvlCursor?: InputMaybe<Scalars['Float']>;
};


export type QueryTransactionNotificationArgs = {
  address: Scalars['String'];
  chain: Chain;
  transactionHash: Scalars['String'];
};


export type QueryV2PairArgs = {
  address: Scalars['String'];
  chain: Chain;
};


export type QueryV2TransactionsArgs = {
  chain: Chain;
  first: Scalars['Int'];
  timestampCursor?: InputMaybe<Scalars['Int']>;
};


export type QueryV3PoolArgs = {
  address: Scalars['String'];
  chain: Chain;
};


export type QueryV3TransactionsArgs = {
  chain: Chain;
  first: Scalars['Int'];
  timestampCursor?: InputMaybe<Scalars['Int']>;
};

export enum SafetyLevel {
  Blocked = 'BLOCKED',
  MediumWarning = 'MEDIUM_WARNING',
  StrongWarning = 'STRONG_WARNING',
  Verified = 'VERIFIED'
}

export type Status = {
  __typename?: 'Status';
  success: Scalars['Boolean'];
};

export type Subscription = {
  __typename?: 'Subscription';
  onAssetActivity?: Maybe<AssetActivity>;
};


export type SubscriptionOnAssetActivityArgs = {
  addresses: Array<Scalars['String']>;
  subscriptionId: Scalars['ID'];
};

export enum SubscriptionType {
  AssetActivity = 'ASSET_ACTIVITY'
}

export type SwapOrderDetails = {
  __typename?: 'SwapOrderDetails';
  encodedOrder: Scalars['String'];
  expiry: Scalars['Int'];
  hash: Scalars['String'];
  id: Scalars['ID'];
  inputToken: Token;
  inputTokenQuantity: Scalars['String'];
  offerer: Scalars['String'];
  outputToken: Token;
  outputTokenQuantity: Scalars['String'];
  /** @deprecated use swapOrderStatus to disambiguate from transactionStatus */
  status: SwapOrderStatus;
  swapOrderStatus: SwapOrderStatus;
  swapOrderType: SwapOrderType;
};

export type SwapOrderDetailsInput = {
  encodedOrder: Scalars['String'];
  expiry: Scalars['Int'];
  hash: Scalars['String'];
  inputAmount: Scalars['String'];
  inputToken: TokenAssetInput;
  offerer: Scalars['String'];
  outputAmount: Scalars['String'];
  outputToken: TokenAssetInput;
  status?: InputMaybe<SwapOrderStatus>;
  swapOrderStatus: SwapOrderStatus;
  swapOrderType: SwapOrderType;
};

export enum SwapOrderStatus {
  Cancelled = 'CANCELLED',
  Error = 'ERROR',
  Expired = 'EXPIRED',
  Filled = 'FILLED',
  InsufficientFunds = 'INSUFFICIENT_FUNDS',
  Open = 'OPEN'
}

export enum SwapOrderType {
  Dutch = 'DUTCH',
  DutchV2 = 'DUTCH_V2',
  Limit = 'LIMIT'
}

export type TimestampedAmount = IAmount & {
  __typename?: 'TimestampedAmount';
  currency?: Maybe<Currency>;
  id: Scalars['ID'];
  timestamp: Scalars['Int'];
  value: Scalars['Float'];
};

export type TimestampedOhlc = {
  __typename?: 'TimestampedOhlc';
  close: Amount;
  high: Amount;
  id: Scalars['ID'];
  low: Amount;
  open: Amount;
  timestamp: Scalars['Int'];
};

export type TimestampedPoolPrice = {
  __typename?: 'TimestampedPoolPrice';
  id: Scalars['ID'];
  timestamp: Scalars['Int'];
  token0Price: Scalars['Float'];
  token1Price: Scalars['Float'];
};

export type Token = IContract & {
  __typename?: 'Token';
  address?: Maybe<Scalars['String']>;
  chain: Chain;
  decimals?: Maybe<Scalars['Int']>;
  feeData?: Maybe<FeeData>;
  id: Scalars['ID'];
  market?: Maybe<TokenMarket>;
  name?: Maybe<Scalars['String']>;
  project?: Maybe<TokenProject>;
  standard?: Maybe<TokenStandard>;
  symbol?: Maybe<Scalars['String']>;
  v2Transactions?: Maybe<Array<Maybe<PoolTransaction>>>;
  v3Transactions?: Maybe<Array<Maybe<PoolTransaction>>>;
};


export type TokenMarketArgs = {
  currency?: InputMaybe<Currency>;
};


export type TokenV2TransactionsArgs = {
  first: Scalars['Int'];
  timestampCursor?: InputMaybe<Scalars['Int']>;
};


export type TokenV3TransactionsArgs = {
  first: Scalars['Int'];
  timestampCursor?: InputMaybe<Scalars['Int']>;
};

export type TokenAmount = {
  __typename?: 'TokenAmount';
  currency: Currency;
  id: Scalars['ID'];
  value: Scalars['String'];
};

export type TokenAmountInput = {
  amount: Scalars['String'];
  token: TokenInput;
};

export type TokenApproval = {
  __typename?: 'TokenApproval';
  approvedAddress: Scalars['String'];
  /**   can be erc20 or native */
  asset: Token;
  id: Scalars['ID'];
  quantity: Scalars['String'];
  tokenStandard: TokenStandard;
};

export type TokenApprovalInput = {
  approvedAddress: Scalars['String'];
  asset: TokenAssetInput;
  quantity: Scalars['String'];
  tokenStandard: TokenStandard;
};

export type TokenAssetInput = {
  address?: InputMaybe<Scalars['String']>;
  chain: Chain;
  decimals?: InputMaybe<Scalars['Int']>;
  name?: InputMaybe<Scalars['String']>;
  standard: TokenStandard;
  symbol?: InputMaybe<Scalars['String']>;
};

export type TokenBalance = {
  __typename?: 'TokenBalance';
  blockNumber?: Maybe<Scalars['Int']>;
  blockTimestamp?: Maybe<Scalars['Int']>;
  denominatedValue?: Maybe<Amount>;
  id: Scalars['ID'];
  isHidden?: Maybe<Scalars['Boolean']>;
  ownerAddress: Scalars['String'];
  quantity?: Maybe<Scalars['Float']>;
  token?: Maybe<Token>;
  tokenProjectMarket?: Maybe<TokenProjectMarket>;
};

export type TokenInput = {
  address: Scalars['String'];
  chainId: Scalars['Int'];
  decimals: Scalars['Int'];
  isNative: Scalars['Boolean'];
};

export type TokenMarket = {
  __typename?: 'TokenMarket';
  fullyDilutedValuation?: Maybe<Amount>;
  historicalTvl?: Maybe<Array<Maybe<TimestampedAmount>>>;
  historicalVolume?: Maybe<Array<Maybe<TimestampedAmount>>>;
  id: Scalars['ID'];
  ohlc?: Maybe<Array<Maybe<TimestampedOhlc>>>;
  price?: Maybe<Amount>;
  priceHighLow?: Maybe<Amount>;
  priceHistory?: Maybe<Array<Maybe<TimestampedAmount>>>;
  pricePercentChange?: Maybe<Amount>;
  priceSource: PriceSource;
  token: Token;
  totalValueLocked?: Maybe<Amount>;
  /**   this volume is cumulative volume over the specified duration */
  volume?: Maybe<Amount>;
};


export type TokenMarketHistoricalTvlArgs = {
  duration: HistoryDuration;
};


export type TokenMarketHistoricalVolumeArgs = {
  duration: HistoryDuration;
};


export type TokenMarketOhlcArgs = {
  duration: HistoryDuration;
};


export type TokenMarketPriceHighLowArgs = {
  duration: HistoryDuration;
  highLow: HighLow;
};


export type TokenMarketPriceHistoryArgs = {
  duration: HistoryDuration;
};


export type TokenMarketPricePercentChangeArgs = {
  duration: HistoryDuration;
};


export type TokenMarketVolumeArgs = {
  duration: HistoryDuration;
};

export type TokenProject = {
  __typename?: 'TokenProject';
  description?: Maybe<Scalars['String']>;
  descriptionTranslations?: Maybe<DescriptionTranslations>;
  homepageUrl?: Maybe<Scalars['String']>;
  id: Scalars['ID'];
  isSpam?: Maybe<Scalars['Boolean']>;
  logo?: Maybe<Image>;
  /** @deprecated use logo */
  logoUrl?: Maybe<Scalars['String']>;
  markets?: Maybe<Array<Maybe<TokenProjectMarket>>>;
  name?: Maybe<Scalars['String']>;
  safetyLevel?: Maybe<SafetyLevel>;
  /** @deprecated use logo */
  smallLogo?: Maybe<Image>;
  spamCode?: Maybe<Scalars['Int']>;
  tokens: Array<Token>;
  twitterName?: Maybe<Scalars['String']>;
};


export type TokenProjectMarketsArgs = {
  currencies: Array<Currency>;
};

export type TokenProjectMarket = {
  __typename?: 'TokenProjectMarket';
  currency: Currency;
  fullyDilutedValuation?: Maybe<Amount>;
  id: Scalars['ID'];
  marketCap?: Maybe<Amount>;
  price?: Maybe<Amount>;
  priceHigh52w?: Maybe<Amount>;
  priceHighLow?: Maybe<Amount>;
  priceHistory?: Maybe<Array<Maybe<TimestampedAmount>>>;
  priceLow52w?: Maybe<Amount>;
  pricePercentChange?: Maybe<Amount>;
  pricePercentChange24h?: Maybe<Amount>;
  tokenProject: TokenProject;
};


export type TokenProjectMarketPriceHighLowArgs = {
  duration: HistoryDuration;
  highLow: HighLow;
};


export type TokenProjectMarketPriceHistoryArgs = {
  duration: HistoryDuration;
};


export type TokenProjectMarketPricePercentChangeArgs = {
  duration: HistoryDuration;
};

export enum TokenSortableField {
  MarketCap = 'MARKET_CAP',
  Popularity = 'POPULARITY',
  TotalValueLocked = 'TOTAL_VALUE_LOCKED',
  Volume = 'VOLUME'
}

export enum TokenStandard {
  Erc20 = 'ERC20',
  Native = 'NATIVE'
}

export type TokenTradeInput = {
  permit?: InputMaybe<PermitInput>;
  routes?: InputMaybe<TokenTradeRoutesInput>;
  slippageToleranceBasisPoints?: InputMaybe<Scalars['Int']>;
  tokenAmount: TokenAmountInput;
};

export type TokenTradeRouteInput = {
  inputAmount: TokenAmountInput;
  outputAmount: TokenAmountInput;
  pools: Array<TradePoolInput>;
};

export type TokenTradeRoutesInput = {
  mixedRoutes?: InputMaybe<Array<TokenTradeRouteInput>>;
  tradeType: TokenTradeType;
  v2Routes?: InputMaybe<Array<TokenTradeRouteInput>>;
  v3Routes?: InputMaybe<Array<TokenTradeRouteInput>>;
};

export enum TokenTradeType {
  ExactInput = 'EXACT_INPUT',
  ExactOutput = 'EXACT_OUTPUT'
}

export type TokenTransfer = {
  __typename?: 'TokenTransfer';
  asset: Token;
  direction: TransactionDirection;
  id: Scalars['ID'];
  quantity: Scalars['String'];
  recipient: Scalars['String'];
  sender: Scalars['String'];
  tokenStandard: TokenStandard;
  transactedValue?: Maybe<Amount>;
};

export type TokenTransferInput = {
  asset: TokenAssetInput;
  direction: TransactionDirection;
  quantity: Scalars['String'];
  recipient: Scalars['String'];
  sender: Scalars['String'];
  tokenStandard: TokenStandard;
  transactedValue?: InputMaybe<AmountInput>;
};

export type TradePoolInput = {
  pair?: InputMaybe<PairInput>;
  pool?: InputMaybe<PoolInput>;
};

export type Transaction = {
  __typename?: 'Transaction';
  blockNumber: Scalars['Int'];
  from: Scalars['String'];
  gasLimit?: Maybe<Scalars['Float']>;
  hash: Scalars['String'];
  id: Scalars['ID'];
  maxFeePerGas?: Maybe<Scalars['Float']>;
  nonce: Scalars['Int'];
  status: TransactionStatus;
  to: Scalars['String'];
};

export type TransactionDetails = {
  __typename?: 'TransactionDetails';
  application?: Maybe<ApplicationContract>;
  assetChanges: Array<Maybe<AssetChange>>;
  from: Scalars['String'];
  hash: Scalars['String'];
  id: Scalars['ID'];
  nonce: Scalars['Int'];
  /** @deprecated use transactionStatus to disambiguate from swapOrderStatus */
  status: TransactionStatus;
  to: Scalars['String'];
  transactionStatus: TransactionStatus;
  type: TransactionType;
};

export type TransactionDetailsInput = {
  application?: InputMaybe<ApplicationContractInput>;
  assetChanges: Array<InputMaybe<AssetChangeInput>>;
  from: Scalars['String'];
  hash: Scalars['String'];
  nonce: Scalars['Int'];
  status?: InputMaybe<TransactionStatus>;
  to: Scalars['String'];
  transactionStatus: TransactionStatus;
  type: TransactionType;
};

export enum TransactionDirection {
  In = 'IN',
  Out = 'OUT',
  Self = 'SELF'
}

export type TransactionNotification = {
  __typename?: 'TransactionNotification';
  hash: Scalars['String'];
  id: Scalars['ID'];
  push: Array<PushNotification>;
};

export enum TransactionStatus {
  Confirmed = 'CONFIRMED',
  Failed = 'FAILED',
  Pending = 'PENDING'
}

export enum TransactionType {
  Approve = 'APPROVE',
  Borrow = 'BORROW',
  Cancel = 'CANCEL',
  Claim = 'CLAIM',
  Deployment = 'DEPLOYMENT',
  Lend = 'LEND',
  Mint = 'MINT',
  Receive = 'RECEIVE',
  Repay = 'REPAY',
  Send = 'SEND',
  Stake = 'STAKE',
  Swap = 'SWAP',
  SwapOrder = 'SWAP_ORDER',
  Unknown = 'UNKNOWN',
  Unstake = 'UNSTAKE',
  Withdraw = 'WITHDRAW'
}

export type V2Pair = IPool & {
  __typename?: 'V2Pair';
  address: Scalars['String'];
  chain: Chain;
  createdAtTimestamp?: Maybe<Scalars['Int']>;
  cumulativeVolume?: Maybe<Amount>;
  historicalVolume?: Maybe<Array<Maybe<TimestampedAmount>>>;
  id: Scalars['ID'];
  priceHistory?: Maybe<Array<Maybe<TimestampedPoolPrice>>>;
  protocolVersion: ProtocolVersion;
  token0?: Maybe<Token>;
  token0Supply?: Maybe<Scalars['Float']>;
  token1?: Maybe<Token>;
  token1Supply?: Maybe<Scalars['Float']>;
  totalLiquidity?: Maybe<Amount>;
  totalLiquidityPercentChange24h?: Maybe<Amount>;
  transactions?: Maybe<Array<Maybe<PoolTransaction>>>;
  txCount?: Maybe<Scalars['Int']>;
};


export type V2PairCumulativeVolumeArgs = {
  duration: HistoryDuration;
};


export type V2PairHistoricalVolumeArgs = {
  duration: HistoryDuration;
};


export type V2PairPriceHistoryArgs = {
  duration: HistoryDuration;
};


export type V2PairTransactionsArgs = {
  first: Scalars['Int'];
  timestampCursor?: InputMaybe<Scalars['Int']>;
};

export type V3Pool = IPool & {
  __typename?: 'V3Pool';
  address: Scalars['String'];
  chain: Chain;
  createdAtTimestamp?: Maybe<Scalars['Int']>;
  cumulativeVolume?: Maybe<Amount>;
  feeTier?: Maybe<Scalars['Float']>;
  historicalVolume?: Maybe<Array<Maybe<TimestampedAmount>>>;
  id: Scalars['ID'];
  priceHistory?: Maybe<Array<Maybe<TimestampedPoolPrice>>>;
  protocolVersion: ProtocolVersion;
  ticks?: Maybe<Array<Maybe<V3PoolTick>>>;
  token0?: Maybe<Token>;
  token0Supply?: Maybe<Scalars['Float']>;
  token1?: Maybe<Token>;
  token1Supply?: Maybe<Scalars['Float']>;
  totalLiquidity?: Maybe<Amount>;
  totalLiquidityPercentChange24h?: Maybe<Amount>;
  transactions?: Maybe<Array<Maybe<PoolTransaction>>>;
  txCount?: Maybe<Scalars['Int']>;
};


export type V3PoolCumulativeVolumeArgs = {
  duration: HistoryDuration;
};


export type V3PoolHistoricalVolumeArgs = {
  duration: HistoryDuration;
};


export type V3PoolPriceHistoryArgs = {
  duration: HistoryDuration;
};


export type V3PoolTicksArgs = {
  first: Scalars['Int'];
  tickIdxCursor?: InputMaybe<Scalars['Int']>;
};


export type V3PoolTransactionsArgs = {
  first: Scalars['Int'];
  timestampCursor?: InputMaybe<Scalars['Int']>;
};

export type V3PoolTick = {
  __typename?: 'V3PoolTick';
  id: Scalars['ID'];
  liquidityGross?: Maybe<Scalars['String']>;
  liquidityNet?: Maybe<Scalars['String']>;
  tickIdx?: Maybe<Scalars['Int']>;
};

export type TokenQueryVariables = Exact<{
  chain: Chain;
  address?: InputMaybe<Scalars['String']>;
}>;


export type TokenQuery = { __typename?: 'Query', token?: { __typename?: 'Token', id: string, decimals?: number, name?: string, chain: Chain, address?: string, symbol?: string, market?: { __typename?: 'TokenMarket', id: string, totalValueLocked?: { __typename?: 'Amount', id: string, value: number, currency?: Currency }, price?: { __typename?: 'Amount', id: string, value: number, currency?: Currency }, volume24H?: { __typename?: 'Amount', id: string, value: number, currency?: Currency }, priceHigh52W?: { __typename?: 'Amount', id: string, value: number }, priceLow52W?: { __typename?: 'Amount', id: string, value: number } }, project?: { __typename?: 'TokenProject', id: string, description?: string, homepageUrl?: string, twitterName?: string, logoUrl?: string, tokens: Array<{ __typename?: 'Token', id: string, chain: Chain, address?: string }> } } };

export type TokenPriceQueryVariables = Exact<{
  chain: Chain;
  address?: InputMaybe<Scalars['String']>;
  duration: HistoryDuration;
}>;


export type TokenPriceQuery = { __typename?: 'Query', token?: { __typename?: 'Token', id: string, address?: string, chain: Chain, market?: { __typename?: 'TokenMarket', id: string, price?: { __typename?: 'Amount', id: string, value: number }, priceHistory?: Array<{ __typename?: 'TimestampedAmount', id: string, timestamp: number, value: number }> } } };

export type TopTokens100QueryVariables = Exact<{
  duration: HistoryDuration;
  chain: Chain;
}>;


export type TopTokens100Query = { __typename?: 'Query', topTokens?: Array<{ __typename?: 'Token', id: string, name?: string, chain: Chain, address?: string, symbol?: string, market?: { __typename?: 'TokenMarket', id: string, totalValueLocked?: { __typename?: 'Amount', id: string, value: number, currency?: Currency }, price?: { __typename?: 'Amount', id: string, value: number, currency?: Currency }, pricePercentChange?: { __typename?: 'Amount', id: string, currency?: Currency, value: number }, volume?: { __typename?: 'Amount', id: string, value: number, currency?: Currency } }, project?: { __typename?: 'TokenProject', id: string, logoUrl?: string } }> };

export type TopTokensSparklineQueryVariables = Exact<{
  duration: HistoryDuration;
  chain: Chain;
}>;


export type TopTokensSparklineQuery = { __typename?: 'Query', topTokens?: Array<{ __typename?: 'Token', id: string, address?: string, chain: Chain, market?: { __typename?: 'TokenMarket', id: string, priceHistory?: Array<{ __typename?: 'TimestampedAmount', id: string, timestamp: number, value: number }> } }> };

export type AssetQueryVariables = Exact<{
  address: Scalars['String'];
  orderBy?: InputMaybe<NftAssetSortableField>;
  asc?: InputMaybe<Scalars['Boolean']>;
  filter?: InputMaybe<NftAssetsFilterInput>;
  first?: InputMaybe<Scalars['Int']>;
  after?: InputMaybe<Scalars['String']>;
  last?: InputMaybe<Scalars['Int']>;
  before?: InputMaybe<Scalars['String']>;
}>;


export type AssetQuery = { __typename?: 'Query', nftAssets?: { __typename?: 'NftAssetConnection', totalCount?: number, edges: Array<{ __typename?: 'NftAssetEdge', cursor: string, node: { __typename?: 'NftAsset', id: string, name?: string, ownerAddress?: string, tokenId: string, description?: string, animationUrl?: string, suspiciousFlag?: boolean, metadataUrl?: string, image?: { __typename?: 'Image', url: string }, smallImage?: { __typename?: 'Image', url: string }, originalImage?: { __typename?: 'Image', url: string }, collection?: { __typename?: 'NftCollection', name?: string, isVerified?: boolean, image?: { __typename?: 'Image', url: string }, creator?: { __typename?: 'NftProfile', address: string, isVerified?: boolean, profileImage?: { __typename?: 'Image', url: string } }, nftContracts?: Array<{ __typename?: 'NftContract', address: string, standard?: NftStandard }> }, listings?: { __typename?: 'NftOrderConnection', edges: Array<{ __typename?: 'NftOrderEdge', cursor: string, node: { __typename?: 'NftOrder', address: string, createdAt: number, endAt?: number, id: string, maker: string, marketplace: NftMarketplace, marketplaceUrl: string, orderHash?: string, quantity: number, startAt: number, status: OrderStatus, taker?: string, tokenId?: string, type: OrderType, protocolParameters?: any, price: { __typename?: 'Amount', currency?: Currency, value: number } } }> }, rarities?: Array<{ __typename?: 'NftAssetRarity', provider?: NftRarityProvider, rank?: number, score?: number }> } }>, pageInfo: { __typename?: 'PageInfo', endCursor?: string, hasNextPage?: boolean, hasPreviousPage?: boolean, startCursor?: string } } };

export type CollectionQueryVariables = Exact<{
  addresses: Array<Scalars['String']> | Scalars['String'];
}>;


export type CollectionQuery = { __typename?: 'Query', nftCollections?: { __typename?: 'NftCollectionConnection', edges: Array<{ __typename?: 'NftCollectionEdge', cursor: string, node: { __typename?: 'NftCollection', collectionId: string, description?: string, discordUrl?: string, homepageUrl?: string, instagramName?: string, isVerified?: boolean, name?: string, numAssets?: number, twitterName?: string, bannerImage?: { __typename?: 'Image', url: string }, image?: { __typename?: 'Image', url: string }, nftContracts?: Array<{ __typename?: 'NftContract', address: string, chain: Chain, name?: string, standard?: NftStandard, symbol?: string, totalSupply?: number }>, traits?: Array<{ __typename?: 'NftCollectionTrait', name?: string, values?: Array<string>, stats?: Array<{ __typename?: 'NftCollectionTraitStats', name?: string, value?: string, assets?: number, listings?: number }> }>, markets?: Array<{ __typename?: 'NftCollectionMarket', owners?: number, floorPrice?: { __typename?: 'TimestampedAmount', currency?: Currency, value: number }, totalVolume?: { __typename?: 'TimestampedAmount', value: number, currency?: Currency }, listings?: { __typename?: 'TimestampedAmount', value: number }, volume?: { __typename?: 'TimestampedAmount', value: number, currency?: Currency }, volumePercentChange?: { __typename?: 'TimestampedAmount', value: number, currency?: Currency }, floorPricePercentChange?: { __typename?: 'TimestampedAmount', value: number, currency?: Currency }, marketplaces?: Array<{ __typename?: 'NftCollectionMarketplace', marketplace?: NftMarketplace, listings?: number, floorPrice?: number }> }> } }>, pageInfo: { __typename?: 'PageInfo', endCursor?: string, hasNextPage?: boolean, hasPreviousPage?: boolean, startCursor?: string } } };

export type DetailsQueryVariables = Exact<{
  address: Scalars['String'];
  tokenId: Scalars['String'];
}>;


export type DetailsQuery = { __typename?: 'Query', nftAssets?: { __typename?: 'NftAssetConnection', edges: Array<{ __typename?: 'NftAssetEdge', node: { __typename?: 'NftAsset', id: string, name?: string, ownerAddress?: string, tokenId: string, description?: string, animationUrl?: string, suspiciousFlag?: boolean, metadataUrl?: string, image?: { __typename?: 'Image', url: string }, smallImage?: { __typename?: 'Image', url: string }, originalImage?: { __typename?: 'Image', url: string }, creator?: { __typename?: 'NftProfile', address: string, isVerified?: boolean, profileImage?: { __typename?: 'Image', url: string } }, collection?: { __typename?: 'NftCollection', name?: string, isVerified?: boolean, numAssets?: number, twitterName?: string, discordUrl?: string, homepageUrl?: string, description?: string, image?: { __typename?: 'Image', url: string }, nftContracts?: Array<{ __typename?: 'NftContract', address: string, standard?: NftStandard }> }, listings?: { __typename?: 'NftOrderConnection', edges: Array<{ __typename?: 'NftOrderEdge', cursor: string, node: { __typename?: 'NftOrder', address: string, createdAt: number, endAt?: number, id: string, maker: string, marketplace: NftMarketplace, marketplaceUrl: string, orderHash?: string, quantity: number, startAt: number, status: OrderStatus, taker?: string, tokenId?: string, type: OrderType, protocolParameters?: any, price: { __typename?: 'Amount', currency?: Currency, value: number } } }> }, rarities?: Array<{ __typename?: 'NftAssetRarity', provider?: NftRarityProvider, rank?: number, score?: number }>, traits?: Array<{ __typename?: 'NftAssetTrait', name?: string, value?: string }> } }> } };

export type NftBalanceQueryVariables = Exact<{
  ownerAddress: Scalars['String'];
  filter?: InputMaybe<NftBalancesFilterInput>;
  first?: InputMaybe<Scalars['Int']>;
  after?: InputMaybe<Scalars['String']>;
  last?: InputMaybe<Scalars['Int']>;
  before?: InputMaybe<Scalars['String']>;
}>;


export type NftBalanceQuery = { __typename?: 'Query', nftBalances?: { __typename?: 'NftBalanceConnection', edges: Array<{ __typename?: 'NftBalanceEdge', node: { __typename?: 'NftBalance', listedMarketplaces?: Array<NftMarketplace>, ownedAsset?: { __typename?: 'NftAsset', id: string, animationUrl?: string, description?: string, flaggedBy?: string, name?: string, ownerAddress?: string, suspiciousFlag?: boolean, tokenId: string, collection?: { __typename?: 'NftCollection', isVerified?: boolean, name?: string, image?: { __typename?: 'Image', url: string }, nftContracts?: Array<{ __typename?: 'NftContract', address: string, chain: Chain, name?: string, standard?: NftStandard, symbol?: string, totalSupply?: number }>, markets?: Array<{ __typename?: 'NftCollectionMarket', floorPrice?: { __typename?: 'TimestampedAmount', value: number } }> }, image?: { __typename?: 'Image', url: string }, originalImage?: { __typename?: 'Image', url: string }, smallImage?: { __typename?: 'Image', url: string }, thumbnail?: { __typename?: 'Image', url: string }, listings?: { __typename?: 'NftOrderConnection', edges: Array<{ __typename?: 'NftOrderEdge', node: { __typename?: 'NftOrder', createdAt: number, marketplace: NftMarketplace, endAt?: number, price: { __typename?: 'Amount', value: number, currency?: Currency } } }> } }, listingFees?: Array<{ __typename?: 'NftFee', payoutAddress: string, basisPoints: number }>, lastPrice?: { __typename?: 'TimestampedAmount', currency?: Currency, timestamp: number, value: number } } }>, pageInfo: { __typename?: 'PageInfo', endCursor?: string, hasNextPage?: boolean, hasPreviousPage?: boolean, startCursor?: string } } };


export const TokenDocument = gql`
    query Token($chain: Chain!, $address: String) {
  token(chain: $chain, address: $address) {
    id
    decimals
    name
    chain
    address
    symbol
    market(currency: USD) {
      id
      totalValueLocked {
        id
        value
        currency
      }
      price {
        id
        value
        currency
      }
      volume24H: volume(duration: DAY) {
        id
        value
        currency
      }
      priceHigh52W: priceHighLow(duration: YEAR, highLow: HIGH) {
        id
        value
      }
      priceLow52W: priceHighLow(duration: YEAR, highLow: LOW) {
        id
        value
      }
    }
    project {
      id
      description
      homepageUrl
      twitterName
      logoUrl
      tokens {
        id
        chain
        address
      }
    }
  }
}
    `;

/**
 * __useTokenQuery__
 *
 * To run a query within a React component, call `useTokenQuery` and pass it any options that fit your needs.
 * When your component renders, `useTokenQuery` returns an object from Apollo Client that contains loading, error, and data properties
 * you can use to render your UI.
 *
 * @param baseOptions options that will be passed into the query, supported options are listed on: https://www.apollographql.com/docs/react/api/react-hooks/#options;
 *
 * @example
 * const { data, loading, error } = useTokenQuery({
 *   variables: {
 *      chain: // value for 'chain'
 *      address: // value for 'address'
 *   },
 * });
 */
export function useTokenQuery(baseOptions: Apollo.QueryHookOptions<TokenQuery, TokenQueryVariables>) {
        const options = {...defaultOptions, ...baseOptions}
        return Apollo.useQuery<TokenQuery, TokenQueryVariables>(TokenDocument, options);
      }
export function useTokenLazyQuery(baseOptions?: Apollo.LazyQueryHookOptions<TokenQuery, TokenQueryVariables>) {
          const options = {...defaultOptions, ...baseOptions}
          return Apollo.useLazyQuery<TokenQuery, TokenQueryVariables>(TokenDocument, options);
        }
export type TokenQueryHookResult = ReturnType<typeof useTokenQuery>;
export type TokenLazyQueryHookResult = ReturnType<typeof useTokenLazyQuery>;
export type TokenQueryResult = Apollo.QueryResult<TokenQuery, TokenQueryVariables>;
export const TokenPriceDocument = gql`
    query TokenPrice($chain: Chain!, $address: String, $duration: HistoryDuration!) {
  token(chain: $chain, address: $address) {
    id
    address
    chain
    market(currency: USD) {
      id
      price {
        id
        value
      }
      priceHistory(duration: $duration) {
        id
        timestamp
        value
      }
    }
  }
}
    `;

/**
 * __useTokenPriceQuery__
 *
 * To run a query within a React component, call `useTokenPriceQuery` and pass it any options that fit your needs.
 * When your component renders, `useTokenPriceQuery` returns an object from Apollo Client that contains loading, error, and data properties
 * you can use to render your UI.
 *
 * @param baseOptions options that will be passed into the query, supported options are listed on: https://www.apollographql.com/docs/react/api/react-hooks/#options;
 *
 * @example
 * const { data, loading, error } = useTokenPriceQuery({
 *   variables: {
 *      chain: // value for 'chain'
 *      address: // value for 'address'
 *      duration: // value for 'duration'
 *   },
 * });
 */
export function useTokenPriceQuery(baseOptions: Apollo.QueryHookOptions<TokenPriceQuery, TokenPriceQueryVariables>) {
        const options = {...defaultOptions, ...baseOptions}
        return Apollo.useQuery<TokenPriceQuery, TokenPriceQueryVariables>(TokenPriceDocument, options);
      }
export function useTokenPriceLazyQuery(baseOptions?: Apollo.LazyQueryHookOptions<TokenPriceQuery, TokenPriceQueryVariables>) {
          const options = {...defaultOptions, ...baseOptions}
          return Apollo.useLazyQuery<TokenPriceQuery, TokenPriceQueryVariables>(TokenPriceDocument, options);
        }
export type TokenPriceQueryHookResult = ReturnType<typeof useTokenPriceQuery>;
export type TokenPriceLazyQueryHookResult = ReturnType<typeof useTokenPriceLazyQuery>;
export type TokenPriceQueryResult = Apollo.QueryResult<TokenPriceQuery, TokenPriceQueryVariables>;
export const TopTokens100Document = gql`
    query TopTokens100($duration: HistoryDuration!, $chain: Chain!) {
  topTokens(pageSize: 100, page: 1, chain: $chain, orderBy: VOLUME) {
    id
    name
    chain
    address
    symbol
    market(currency: USD) {
      id
      totalValueLocked {
        id
        value
        currency
      }
      price {
        id
        value
        currency
      }
      pricePercentChange(duration: $duration) {
        id
        currency
        value
      }
      volume(duration: $duration) {
        id
        value
        currency
      }
    }
    project {
      id
      logoUrl
    }
  }
}
    `;

/**
 * __useTopTokens100Query__
 *
 * To run a query within a React component, call `useTopTokens100Query` and pass it any options that fit your needs.
 * When your component renders, `useTopTokens100Query` returns an object from Apollo Client that contains loading, error, and data properties
 * you can use to render your UI.
 *
 * @param baseOptions options that will be passed into the query, supported options are listed on: https://www.apollographql.com/docs/react/api/react-hooks/#options;
 *
 * @example
 * const { data, loading, error } = useTopTokens100Query({
 *   variables: {
 *      duration: // value for 'duration'
 *      chain: // value for 'chain'
 *   },
 * });
 */
export function useTopTokens100Query(baseOptions: Apollo.QueryHookOptions<TopTokens100Query, TopTokens100QueryVariables>) {
        const options = {...defaultOptions, ...baseOptions}
        return Apollo.useQuery<TopTokens100Query, TopTokens100QueryVariables>(TopTokens100Document, options);
      }
export function useTopTokens100LazyQuery(baseOptions?: Apollo.LazyQueryHookOptions<TopTokens100Query, TopTokens100QueryVariables>) {
          const options = {...defaultOptions, ...baseOptions}
          return Apollo.useLazyQuery<TopTokens100Query, TopTokens100QueryVariables>(TopTokens100Document, options);
        }
export type TopTokens100QueryHookResult = ReturnType<typeof useTopTokens100Query>;
export type TopTokens100LazyQueryHookResult = ReturnType<typeof useTopTokens100LazyQuery>;
export type TopTokens100QueryResult = Apollo.QueryResult<TopTokens100Query, TopTokens100QueryVariables>;
export const TopTokensSparklineDocument = gql`
    query TopTokensSparkline($duration: HistoryDuration!, $chain: Chain!) {
  topTokens(pageSize: 100, page: 1, chain: $chain) {
    id
    address
    chain
    market(currency: USD) {
      id
      priceHistory(duration: $duration) {
        id
        timestamp
        value
      }
    }
  }
}
    `;

/**
 * __useTopTokensSparklineQuery__
 *
 * To run a query within a React component, call `useTopTokensSparklineQuery` and pass it any options that fit your needs.
 * When your component renders, `useTopTokensSparklineQuery` returns an object from Apollo Client that contains loading, error, and data properties
 * you can use to render your UI.
 *
 * @param baseOptions options that will be passed into the query, supported options are listed on: https://www.apollographql.com/docs/react/api/react-hooks/#options;
 *
 * @example
 * const { data, loading, error } = useTopTokensSparklineQuery({
 *   variables: {
 *      duration: // value for 'duration'
 *      chain: // value for 'chain'
 *   },
 * });
 */
export function useTopTokensSparklineQuery(baseOptions: Apollo.QueryHookOptions<TopTokensSparklineQuery, TopTokensSparklineQueryVariables>) {
        const options = {...defaultOptions, ...baseOptions}
        return Apollo.useQuery<TopTokensSparklineQuery, TopTokensSparklineQueryVariables>(TopTokensSparklineDocument, options);
      }
export function useTopTokensSparklineLazyQuery(baseOptions?: Apollo.LazyQueryHookOptions<TopTokensSparklineQuery, TopTokensSparklineQueryVariables>) {
          const options = {...defaultOptions, ...baseOptions}
          return Apollo.useLazyQuery<TopTokensSparklineQuery, TopTokensSparklineQueryVariables>(TopTokensSparklineDocument, options);
        }
export type TopTokensSparklineQueryHookResult = ReturnType<typeof useTopTokensSparklineQuery>;
export type TopTokensSparklineLazyQueryHookResult = ReturnType<typeof useTopTokensSparklineLazyQuery>;
export type TopTokensSparklineQueryResult = Apollo.QueryResult<TopTokensSparklineQuery, TopTokensSparklineQueryVariables>;
export const AssetDocument = gql`
    query Asset($address: String!, $orderBy: NftAssetSortableField, $asc: Boolean, $filter: NftAssetsFilterInput, $first: Int, $after: String, $last: Int, $before: String) {
  nftAssets(
    address: $address
    orderBy: $orderBy
    asc: $asc
    filter: $filter
    first: $first
    after: $after
    last: $last
    before: $before
  ) {
    edges {
      node {
        id
        name
        ownerAddress
        image {
          url
        }
        smallImage {
          url
        }
        originalImage {
          url
        }
        tokenId
        description
        animationUrl
        suspiciousFlag
        collection {
          name
          isVerified
          image {
            url
          }
          creator {
            address
            profileImage {
              url
            }
            isVerified
          }
          nftContracts {
            address
            standard
          }
        }
        listings(first: 1) {
          edges {
            node {
              address
              createdAt
              endAt
              id
              maker
              marketplace
              marketplaceUrl
              orderHash
              price {
                currency
                value
              }
              quantity
              startAt
              status
              taker
              tokenId
              type
              protocolParameters
            }
            cursor
          }
        }
        rarities {
          provider
          rank
          score
        }
        metadataUrl
      }
      cursor
    }
    totalCount
    pageInfo {
      endCursor
      hasNextPage
      hasPreviousPage
      startCursor
    }
  }
}
    `;

/**
 * __useAssetQuery__
 *
 * To run a query within a React component, call `useAssetQuery` and pass it any options that fit your needs.
 * When your component renders, `useAssetQuery` returns an object from Apollo Client that contains loading, error, and data properties
 * you can use to render your UI.
 *
 * @param baseOptions options that will be passed into the query, supported options are listed on: https://www.apollographql.com/docs/react/api/react-hooks/#options;
 *
 * @example
 * const { data, loading, error } = useAssetQuery({
 *   variables: {
 *      address: // value for 'address'
 *      orderBy: // value for 'orderBy'
 *      asc: // value for 'asc'
 *      filter: // value for 'filter'
 *      first: // value for 'first'
 *      after: // value for 'after'
 *      last: // value for 'last'
 *      before: // value for 'before'
 *   },
 * });
 */
export function useAssetQuery(baseOptions: Apollo.QueryHookOptions<AssetQuery, AssetQueryVariables>) {
        const options = {...defaultOptions, ...baseOptions}
        return Apollo.useQuery<AssetQuery, AssetQueryVariables>(AssetDocument, options);
      }
export function useAssetLazyQuery(baseOptions?: Apollo.LazyQueryHookOptions<AssetQuery, AssetQueryVariables>) {
          const options = {...defaultOptions, ...baseOptions}
          return Apollo.useLazyQuery<AssetQuery, AssetQueryVariables>(AssetDocument, options);
        }
export type AssetQueryHookResult = ReturnType<typeof useAssetQuery>;
export type AssetLazyQueryHookResult = ReturnType<typeof useAssetLazyQuery>;
export type AssetQueryResult = Apollo.QueryResult<AssetQuery, AssetQueryVariables>;
export const CollectionDocument = gql`
    query Collection($addresses: [String!]!) {
  nftCollections(filter: {addresses: $addresses}) {
    edges {
      cursor
      node {
        bannerImage {
          url
        }
        collectionId
        description
        discordUrl
        homepageUrl
        image {
          url
        }
        instagramName
        isVerified
        name
        numAssets
        twitterName
        nftContracts {
          address
          chain
          name
          standard
          symbol
          totalSupply
        }
        traits {
          name
          values
          stats {
            name
            value
            assets
            listings
          }
        }
        markets(currencies: ETH) {
          floorPrice {
            currency
            value
          }
          owners
          totalVolume {
            value
            currency
          }
          listings {
            value
          }
          volume(duration: DAY) {
            value
            currency
          }
          volumePercentChange(duration: DAY) {
            value
            currency
          }
          floorPricePercentChange(duration: DAY) {
            value
            currency
          }
          marketplaces {
            marketplace
            listings
            floorPrice
          }
        }
      }
    }
    pageInfo {
      endCursor
      hasNextPage
      hasPreviousPage
      startCursor
    }
  }
}
    `;

/**
 * __useCollectionQuery__
 *
 * To run a query within a React component, call `useCollectionQuery` and pass it any options that fit your needs.
 * When your component renders, `useCollectionQuery` returns an object from Apollo Client that contains loading, error, and data properties
 * you can use to render your UI.
 *
 * @param baseOptions options that will be passed into the query, supported options are listed on: https://www.apollographql.com/docs/react/api/react-hooks/#options;
 *
 * @example
 * const { data, loading, error } = useCollectionQuery({
 *   variables: {
 *      addresses: // value for 'addresses'
 *   },
 * });
 */
export function useCollectionQuery(baseOptions: Apollo.QueryHookOptions<CollectionQuery, CollectionQueryVariables>) {
        const options = {...defaultOptions, ...baseOptions}
        return Apollo.useQuery<CollectionQuery, CollectionQueryVariables>(CollectionDocument, options);
      }
export function useCollectionLazyQuery(baseOptions?: Apollo.LazyQueryHookOptions<CollectionQuery, CollectionQueryVariables>) {
          const options = {...defaultOptions, ...baseOptions}
          return Apollo.useLazyQuery<CollectionQuery, CollectionQueryVariables>(CollectionDocument, options);
        }
export type CollectionQueryHookResult = ReturnType<typeof useCollectionQuery>;
export type CollectionLazyQueryHookResult = ReturnType<typeof useCollectionLazyQuery>;
export type CollectionQueryResult = Apollo.QueryResult<CollectionQuery, CollectionQueryVariables>;
export const DetailsDocument = gql`
    query Details($address: String!, $tokenId: String!) {
  nftAssets(address: $address, filter: {listed: false, tokenIds: [$tokenId]}) {
    edges {
      node {
        id
        name
        ownerAddress
        image {
          url
        }
        smallImage {
          url
        }
        originalImage {
          url
        }
        tokenId
        description
        animationUrl
        suspiciousFlag
        creator {
          address
          profileImage {
            url
          }
          isVerified
        }
        collection {
          name
          isVerified
          numAssets
          twitterName
          discordUrl
          homepageUrl
          image {
            url
          }
          nftContracts {
            address
            standard
          }
          description
        }
        listings(first: 1) {
          edges {
            node {
              address
              createdAt
              endAt
              id
              maker
              marketplace
              marketplaceUrl
              orderHash
              price {
                currency
                value
              }
              quantity
              startAt
              status
              taker
              tokenId
              type
              protocolParameters
            }
            cursor
          }
        }
        rarities {
          provider
          rank
          score
        }
        metadataUrl
        traits {
          name
          value
        }
      }
    }
  }
}
    `;

/**
 * __useDetailsQuery__
 *
 * To run a query within a React component, call `useDetailsQuery` and pass it any options that fit your needs.
 * When your component renders, `useDetailsQuery` returns an object from Apollo Client that contains loading, error, and data properties
 * you can use to render your UI.
 *
 * @param baseOptions options that will be passed into the query, supported options are listed on: https://www.apollographql.com/docs/react/api/react-hooks/#options;
 *
 * @example
 * const { data, loading, error } = useDetailsQuery({
 *   variables: {
 *      address: // value for 'address'
 *      tokenId: // value for 'tokenId'
 *   },
 * });
 */
export function useDetailsQuery(baseOptions: Apollo.QueryHookOptions<DetailsQuery, DetailsQueryVariables>) {
        const options = {...defaultOptions, ...baseOptions}
        return Apollo.useQuery<DetailsQuery, DetailsQueryVariables>(DetailsDocument, options);
      }
export function useDetailsLazyQuery(baseOptions?: Apollo.LazyQueryHookOptions<DetailsQuery, DetailsQueryVariables>) {
          const options = {...defaultOptions, ...baseOptions}
          return Apollo.useLazyQuery<DetailsQuery, DetailsQueryVariables>(DetailsDocument, options);
        }
export type DetailsQueryHookResult = ReturnType<typeof useDetailsQuery>;
export type DetailsLazyQueryHookResult = ReturnType<typeof useDetailsLazyQuery>;
export type DetailsQueryResult = Apollo.QueryResult<DetailsQuery, DetailsQueryVariables>;
export const NftBalanceDocument = gql`
    query NftBalance($ownerAddress: String!, $filter: NftBalancesFilterInput, $first: Int, $after: String, $last: Int, $before: String) {
  nftBalances(
    ownerAddress: $ownerAddress
    filter: $filter
    first: $first
    after: $after
    last: $last
    before: $before
  ) {
    edges {
      node {
        ownedAsset {
          id
          animationUrl
          collection {
            isVerified
            image {
              url
            }
            name
            nftContracts {
              address
              chain
              name
              standard
              symbol
              totalSupply
            }
            markets(currencies: ETH) {
              floorPrice {
                value
              }
            }
          }
          description
          flaggedBy
          image {
            url
          }
          originalImage {
            url
          }
          name
          ownerAddress
          smallImage {
            url
          }
          suspiciousFlag
          tokenId
          thumbnail {
            url
          }
          listings(first: 1) {
            edges {
              node {
                price {
                  value
                  currency
                }
                createdAt
                marketplace
                endAt
              }
            }
          }
        }
        listedMarketplaces
        listingFees {
          payoutAddress
          basisPoints
        }
        lastPrice {
          currency
          timestamp
          value
        }
      }
    }
    pageInfo {
      endCursor
      hasNextPage
      hasPreviousPage
      startCursor
    }
  }
}
    `;

/**
 * __useNftBalanceQuery__
 *
 * To run a query within a React component, call `useNftBalanceQuery` and pass it any options that fit your needs.
 * When your component renders, `useNftBalanceQuery` returns an object from Apollo Client that contains loading, error, and data properties
 * you can use to render your UI.
 *
 * @param baseOptions options that will be passed into the query, supported options are listed on: https://www.apollographql.com/docs/react/api/react-hooks/#options;
 *
 * @example
 * const { data, loading, error } = useNftBalanceQuery({
 *   variables: {
 *      ownerAddress: // value for 'ownerAddress'
 *      filter: // value for 'filter'
 *      first: // value for 'first'
 *      after: // value for 'after'
 *      last: // value for 'last'
 *      before: // value for 'before'
 *   },
 * });
 */
export function useNftBalanceQuery(baseOptions: Apollo.QueryHookOptions<NftBalanceQuery, NftBalanceQueryVariables>) {
        const options = {...defaultOptions, ...baseOptions}
        return Apollo.useQuery<NftBalanceQuery, NftBalanceQueryVariables>(NftBalanceDocument, options);
      }
export function useNftBalanceLazyQuery(baseOptions?: Apollo.LazyQueryHookOptions<NftBalanceQuery, NftBalanceQueryVariables>) {
          const options = {...defaultOptions, ...baseOptions}
          return Apollo.useLazyQuery<NftBalanceQuery, NftBalanceQueryVariables>(NftBalanceDocument, options);
        }
export type NftBalanceQueryHookResult = ReturnType<typeof useNftBalanceQuery>;
export type NftBalanceLazyQueryHookResult = ReturnType<typeof useNftBalanceLazyQuery>;
export type NftBalanceQueryResult = Apollo.QueryResult<NftBalanceQuery, NftBalanceQueryVariables>;