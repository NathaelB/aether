Feature: Downtime intervals

  @spec-down-1
  Scenario: A run of failed checks followed by a reachable one is one closed interval
    Given a reachable check at 0 seconds
    And a failed check at 30 seconds
    And a failed check at 60 seconds
    And a reachable check at 90 seconds
    When downtime intervals are derived
    Then there is 1 downtime interval
    And downtime interval 1 starts at 30 seconds and ends at 90 seconds, lasting 60 seconds

  @spec-down-2
  Scenario: A deployment still failing has an open interval
    Given a reachable check at 0 seconds
    And a failed check at 30 seconds
    And a failed check at 60 seconds
    When downtime intervals are derived
    Then there is 1 downtime interval
    And downtime interval 1 starts at 30 seconds and is still open

  @spec-down-3
  Scenario: Two separate outages are two intervals
    Given a failed check at 0 seconds
    And a reachable check at 30 seconds
    And a reachable check at 60 seconds
    And a failed check at 90 seconds
    And a reachable check at 120 seconds
    When downtime intervals are derived
    Then there are 2 downtime intervals
    And downtime interval 1 starts at 0 seconds and ends at 30 seconds, lasting 30 seconds
    And downtime interval 2 starts at 90 seconds and ends at 120 seconds, lasting 30 seconds

  @spec-down-4
  Scenario: A single failed check between reachable ones is one interval
    Given a reachable check at 0 seconds
    And a failed check at 30 seconds
    And a reachable check at 60 seconds
    When downtime intervals are derived
    Then there is 1 downtime interval
    And downtime interval 1 starts at 30 seconds and ends at 60 seconds, lasting 30 seconds

  @spec-down-5
  Scenario: Reachable checks only have no interval
    Given a reachable check at 0 seconds
    And a reachable check at 30 seconds
    When downtime intervals are derived
    Then there are 0 downtime intervals

  @spec-down-6
  Scenario: A failure and a recovery a long gap apart are one interval that availability leaves unobserved
    Given a failed check at 0 seconds
    And a reachable check at 1000 seconds
    When downtime intervals are derived
    Then there is 1 downtime interval
    And downtime interval 1 starts at 0 seconds and ends at 1000 seconds, lasting 1000 seconds
    And availability at 1000 seconds over a window of 1000 seconds has no percentage
