Feature: Availability

  @spec-avail-1
  Scenario: A deployment reachable on every check is fully available
    Given a deployment checked every 1 minute for 30 days
    When availability is computed over the last 30 days
    Then availability is 100 percent
    And the window coverage is full

  @spec-avail-2
  Scenario: Four minutes of downtime in 30 days
    Given a deployment checked every 1 minute for 30 days
    And it was down for 4 minutes after 11 days
    When availability is computed over the last 30 days
    Then availability is 99.99074 percent
    And the window coverage is full

  @spec-avail-3
  Scenario: A gap longer than five minutes between two checks is unobserved
    Given a reachable check at 0 seconds
    And a reachable check at 60 seconds
    And a failed check at 1000 seconds
    And a reachable check at 1060 seconds
    When availability is computed at 1060 seconds over a window of 1060 seconds
    Then availability is 50 percent

  @spec-avail-3
  Scenario Outline: The five minute limit on the gap between two checks
    Given a failed check at 0 seconds
    And a reachable check at <gap> seconds
    When availability is computed at <gap> seconds over a window of <gap> seconds
    Then <outcome>

    Examples:
      | gap | outcome                                |
      | 300 | availability is 0 percent              |
      | 301 | there is no availability percentage    |

  @spec-avail-4
  Scenario: A window with nothing observed has no percentage
    When availability is computed at 100 seconds over a window of 100 seconds
    Then there is no availability percentage
    And the window coverage is partial

  @spec-avail-4
  Scenario: A single check older than the window leaves nothing observed
    Given a reachable check at 0 seconds
    When availability is computed at 10000 seconds over a window of 10000 seconds
    Then there is no availability percentage

  @spec-avail-5
  Scenario: A deployment still failing at the end counts the open time as downtime
    Given a reachable check at 0 seconds
    And a reachable check at 60 seconds
    And a failed check at 120 seconds
    When availability is computed at 180 seconds over a window of 180 seconds
    Then availability is 66.66667 percent

  @spec-avail-6
  Scenario Outline: The window is covered when its first and last checks are within five minutes of its ends
    Given a reachable check at <first> seconds
    And a reachable check at <last> seconds
    When availability is computed at 3600 seconds over a window of 3600 seconds
    Then the window coverage is <coverage>

    Examples:
      | first | last | coverage |
      | 0     | 3600 | full     |
      | 300   | 3600 | full     |
      | 301   | 3600 | partial  |
      | 0     | 3300 | full     |
      | 0     | 3299 | partial  |

  @spec-avail-6
  Scenario: A window longer than the history is not covered
    Given a deployment checked every 1 minute for 1 day
    When availability is computed at 86400 seconds over a window of 259200 seconds
    Then availability is 100 percent
    And the window coverage is partial
