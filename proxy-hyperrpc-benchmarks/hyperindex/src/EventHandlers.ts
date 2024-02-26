/*
 *Please refer to https://docs.envio.dev for a thorough guide on all Envio indexer features*
 */
import {
  RocketTokenRETHContract_Approval_loader,
  RocketTokenRETHContract_Approval_handler,
  RocketTokenRETHContract_Transfer_loader,
  RocketTokenRETHContract_Transfer_handler,
} from "../generated/src/Handlers.gen";

import {
  RocketTokenRETH_ApprovalEntity,
  RocketTokenRETH_TransferEntity,
  EventsSummaryEntity,
} from "../generated/src/Types.gen";

export const GLOBAL_EVENTS_SUMMARY_KEY = "GlobalEventsSummary";

const endBlock = 19000000;
let onlyOnce = false;
let startTimestamp: number;

let printOnce = () => {
  if (!onlyOnce) {
    onlyOnce = true;
    let now = Date.now();
    startTimestamp = now;
    console.log("The first event was indexed");
    console.log(`Start timestamp: ${now}`);
  }
};

let stopIndexingAtEndBlock = (blockNumber: number) => {
  if (endBlock < blockNumber) {
    let now = Date.now();
    console.log(`We've reached block ${blockNumber}`);
    console.log(`End timestamp: ${now}`);
    console.log(`Elapsed time: ${now - startTimestamp}ms`);
    process.exit(0);
  }
};

const INITIAL_EVENTS_SUMMARY: EventsSummaryEntity = {
  id: GLOBAL_EVENTS_SUMMARY_KEY,
  rocketTokenRETH_ApprovalCount: BigInt(0),
  rocketTokenRETH_TransferCount: BigInt(0),
};

RocketTokenRETHContract_Approval_loader(({ event, context }) => {
  context.EventsSummary.load(GLOBAL_EVENTS_SUMMARY_KEY);
});

RocketTokenRETHContract_Approval_handler(({ event, context }) => {
  printOnce();
  stopIndexingAtEndBlock(event.blockNumber);

  const summary = context.EventsSummary.get(GLOBAL_EVENTS_SUMMARY_KEY);

  const currentSummaryEntity: EventsSummaryEntity =
    summary ?? INITIAL_EVENTS_SUMMARY;

  const nextSummaryEntity = {
    ...currentSummaryEntity,
    rocketTokenRETH_ApprovalCount:
      currentSummaryEntity.rocketTokenRETH_ApprovalCount + BigInt(1),
  };

  const rocketTokenRETH_ApprovalEntity: RocketTokenRETH_ApprovalEntity = {
    id: event.transactionHash + event.logIndex.toString(),
    owner: event.params.owner,
    spender: event.params.spender,
    value: event.params.value,
    eventsSummary: GLOBAL_EVENTS_SUMMARY_KEY,
  };

  context.EventsSummary.set(nextSummaryEntity);
  context.RocketTokenRETH_Approval.set(rocketTokenRETH_ApprovalEntity);
});
RocketTokenRETHContract_Transfer_loader(({ event, context }) => {
  context.EventsSummary.load(GLOBAL_EVENTS_SUMMARY_KEY);
});

RocketTokenRETHContract_Transfer_handler(({ event, context }) => {
  printOnce();
  stopIndexingAtEndBlock(event.blockNumber);

  const summary = context.EventsSummary.get(GLOBAL_EVENTS_SUMMARY_KEY);

  const currentSummaryEntity: EventsSummaryEntity =
    summary ?? INITIAL_EVENTS_SUMMARY;

  const nextSummaryEntity = {
    ...currentSummaryEntity,
    rocketTokenRETH_TransferCount:
      currentSummaryEntity.rocketTokenRETH_TransferCount + BigInt(1),
  };

  const rocketTokenRETH_TransferEntity: RocketTokenRETH_TransferEntity = {
    id: event.transactionHash + event.logIndex.toString(),
    from: event.params.from,
    to: event.params.to,
    value: event.params.value,
    eventsSummary: GLOBAL_EVENTS_SUMMARY_KEY,
  };

  context.EventsSummary.set(nextSummaryEntity);
  context.RocketTokenRETH_Transfer.set(rocketTokenRETH_TransferEntity);
});
