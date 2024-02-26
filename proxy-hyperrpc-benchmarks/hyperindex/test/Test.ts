import assert = require("assert")
import { MockDb, RocketTokenRETH } from "../generated/src/TestHelpers.gen";
import {
  EventsSummaryEntity,
  RocketTokenRETH_ApprovalEntity,
} from "../generated/src/Types.gen";

import { Addresses } from "../generated/src/bindings/Ethers.bs";

import { GLOBAL_EVENTS_SUMMARY_KEY } from "../src/EventHandlers";


const MOCK_EVENTS_SUMMARY_ENTITY: EventsSummaryEntity = {
  id: GLOBAL_EVENTS_SUMMARY_KEY,
  rocketTokenRETH_ApprovalCount: BigInt(0),
  rocketTokenRETH_TransferCount: BigInt(0),
};

describe("RocketTokenRETH contract Approval event tests", () => {
  // Create mock db
  const mockDbInitial = MockDb.createMockDb();

  // Add mock EventsSummaryEntity to mock db
  const mockDbFinal = mockDbInitial.entities.EventsSummary.set(
    MOCK_EVENTS_SUMMARY_ENTITY
  );

  // Creating mock RocketTokenRETH contract Approval event
  const mockRocketTokenRETHApprovalEvent = RocketTokenRETH.Approval.createMockEvent({
    owner: Addresses.defaultAddress,
    spender: Addresses.defaultAddress,
    value: 0n,
    mockEventData: {
      chainId: 1,
      blockNumber: 0,
      blockTimestamp: 0,
      blockHash: "0x0000000000000000000000000000000000000000000000000000000000000000",
      srcAddress: Addresses.defaultAddress,
      transactionHash: "0x0000000000000000000000000000000000000000000000000000000000000000",
      transactionIndex: 0,
      logIndex: 0,
    },
  });

  // Processing the event
  const mockDbUpdated = RocketTokenRETH.Approval.processEvent({
    event: mockRocketTokenRETHApprovalEvent,
    mockDb: mockDbFinal,
  });

  it("RocketTokenRETH_ApprovalEntity is created correctly", () => {
    // Getting the actual entity from the mock database
    let actualRocketTokenRETHApprovalEntity = mockDbUpdated.entities.RocketTokenRETH_Approval.get(
      mockRocketTokenRETHApprovalEvent.transactionHash +
        mockRocketTokenRETHApprovalEvent.logIndex.toString()
    );

    // Creating the expected entity
    const expectedRocketTokenRETHApprovalEntity: RocketTokenRETH_ApprovalEntity = {
      id:
        mockRocketTokenRETHApprovalEvent.transactionHash +
        mockRocketTokenRETHApprovalEvent.logIndex.toString(),
      owner: mockRocketTokenRETHApprovalEvent.params.owner,
      spender: mockRocketTokenRETHApprovalEvent.params.spender,
      value: mockRocketTokenRETHApprovalEvent.params.value,
      eventsSummary: "GlobalEventsSummary",
    };
    // Asserting that the entity in the mock database is the same as the expected entity
    assert.deepEqual(actualRocketTokenRETHApprovalEntity, expectedRocketTokenRETHApprovalEntity, "Actual RocketTokenRETHApprovalEntity should be the same as the expectedRocketTokenRETHApprovalEntity");
  });

  it("EventsSummaryEntity is updated correctly", () => {
    // Getting the actual entity from the mock database
    let actualEventsSummaryEntity = mockDbUpdated.entities.EventsSummary.get(
      GLOBAL_EVENTS_SUMMARY_KEY
    );

    // Creating the expected entity
    const expectedEventsSummaryEntity: EventsSummaryEntity = {
      ...MOCK_EVENTS_SUMMARY_ENTITY,
      rocketTokenRETH_ApprovalCount: MOCK_EVENTS_SUMMARY_ENTITY.rocketTokenRETH_ApprovalCount + BigInt(1),
    };
    // Asserting that the entity in the mock database is the same as the expected entity
    assert.deepEqual(actualEventsSummaryEntity, expectedEventsSummaryEntity, "Actual RocketTokenRETHApprovalEntity should be the same as the expectedRocketTokenRETHApprovalEntity");
  });
});
