Feature: Signals

  The heartbeat probe turns a silent data plane into a signal, and a data
  plane that reports again into a closed one. A condition seen again does not
  make a second signal.

  @spec-sig-1
  Scenario: A silent data plane opens one signal
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that last reported 600 seconds ago
    When the heartbeat probe ticks
    Then there is 1 open signal
    And the open signal is a "dataplane.heartbeat_stale" about "alpha"

  @spec-sig-2
  Scenario: The same condition on the next tick updates the same signal
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that last reported 600 seconds ago
    And the heartbeat probe has ticked
    And 30 seconds pass
    When the heartbeat probe ticks
    Then there is 1 signal in all
    And the open signal was opened at the first tick and last seen 30 seconds later

  @spec-sig-3
  Scenario: A heartbeat after the silence closes the signal
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that last reported 600 seconds ago
    And the heartbeat probe has ticked
    And 30 seconds pass
    And "alpha" sends a heartbeat
    When the heartbeat probe ticks
    Then there is no open signal
    And there is 1 closed signal

  @spec-sig-4
  Scenario: A condition that returns after a close opens a new signal
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that last reported 600 seconds ago
    And the heartbeat probe has ticked
    And "alpha" sends a heartbeat
    And the heartbeat probe has ticked
    And 600 seconds pass
    When the heartbeat probe ticks
    Then there are 2 signals in all
    And there is 1 open signal
    And there is 1 closed signal
    And the open signal is not the closed one

  @spec-sig-5
  Scenario: A data plane never seen opens nothing
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that was never seen
    When the heartbeat probe ticks
    Then there are 0 signals in all

  @spec-sig-5
  Scenario: A data plane never seen closes a leftover signal of its own
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that was never seen
    And a heartbeat stale signal left open for "alpha"
    When the heartbeat probe ticks
    Then there is no open signal
    And there is 1 closed signal

  @spec-sig-6
  Scenario Outline: Liveness around the heartbeat window
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that last reported <age> seconds ago
    When the liveness of "alpha" is derived
    Then "alpha" is <liveness>

    Examples:
      | age | liveness    |
      | 89  | reachable   |
      | 90  | reachable   |
      | 91  | unreachable |

  @spec-sig-6
  Scenario: A data plane that never reported has no liveness to judge
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that was never seen
    When the liveness of "alpha" is derived
    Then "alpha" is never seen

  @spec-sig-7
  Scenario: Only open signals are listed
    Given a heartbeat window of 90 seconds
    And an operator who holds the right to view the estate
    And a data plane "alpha" that last reported 600 seconds ago
    And a data plane "beta" that last reported 600 seconds ago
    And the heartbeat probe has ticked
    And "beta" sends a heartbeat
    And the heartbeat probe has ticked
    When the caller lists the open signals
    Then the listing shows 1 signal
    And the listing shows a signal about "alpha"

  @spec-sig-7
  Scenario: A caller without the right to view the estate is refused
    Given a heartbeat window of 90 seconds
    And a user who holds no platform right
    And a data plane "alpha" that last reported 600 seconds ago
    And the heartbeat probe has ticked
    When the caller lists the open signals
    Then the listing is refused because the right to view the estate is missing

  @spec-sig-8
  Scenario: Two silent data planes each get their own signal
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that last reported 600 seconds ago
    And a data plane "beta" that last reported 600 seconds ago
    When the heartbeat probe ticks
    Then there are 2 open signals

  @spec-sig-8
  Scenario: A signal of another kind about another subject does not dedup into a heartbeat signal
    Given a heartbeat window of 90 seconds
    And a data plane "alpha" that last reported 600 seconds ago
    And the heartbeat probe has ticked
    When a "deployment.unreachable" signal is written for the deployment "web"
    Then there are 2 open signals
    And the open signal is a "dataplane.heartbeat_stale" about "alpha"
    And the open signal is a "deployment.unreachable" about "web"
