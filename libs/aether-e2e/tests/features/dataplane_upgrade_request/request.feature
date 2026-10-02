Feature: Requesting a data plane upgrade

  An operator asks for data planes to move to a release. The request is
  checked in full before any upgrade action is created.

  @spec-req-1
  Scenario: A request creates one upgrade action per targeted data plane
    Given an operator who holds the right to operate the fleet
    And the minimum Herald version is "26.0.0"
    And data planes "alpha, beta, gamma" whose Herald reports "26.1.0"
    When the operator requests an upgrade to "26.2.0" of the data planes "alpha, beta"
    Then 2 actions are created
    And a dataplane.upgrade action addressed to "alpha" with no deployment exists
    And a dataplane.upgrade action addressed to "beta" with no deployment exists
    And no action is addressed to "gamma"

  @spec-req-2
  Scenario: A user without the right to operate the fleet is refused
    Given a user who holds no platform right
    And the minimum Herald version is "26.0.0"
    And data planes "alpha" whose Herald reports "26.1.0"
    When the operator requests an upgrade to "26.2.0" of the data planes "alpha"
    Then the request is refused because "the right to operate the fleet is missing"
    And nothing is created

  @spec-req-3
  Scenario Outline: An invalid request is refused even when its targets are valid
    Given an operator who holds the right to operate the fleet
    And the minimum Herald version is "26.0.0"
    And data planes "alpha, beta" whose Herald reports "26.1.0"
    When the operator requests an upgrade of the data planes "alpha, beta" to "<version>" of components "<components>" by strategy "<strategy>" with max_unavailable <max_unavailable>
    Then the request is refused because "the payload is invalid"
    And nothing is created

    Examples:
      | case                | version | components | strategy   | max_unavailable |
      | no component        | 26.2.0  |            | rolling    | 1               |
      | nothing may go down | 26.2.0  | All        | rolling    | 0               |
      | an unknown strategy | 26.2.0  | All        | blue-green | 1               |
      | not a version       | latest  | All        | rolling    | 1               |

  @spec-req-4
  Scenario: An unknown data plane refuses the whole request
    Given an operator who holds the right to operate the fleet
    And the minimum Herald version is "26.0.0"
    And data planes "alpha, beta" whose Herald reports "26.1.0"
    When the operator requests an upgrade to "26.2.0" of the data planes "alpha, ghost, beta"
    Then the request is refused because "the data plane is unknown"
    And nothing is created

  @spec-req-5
  Scenario Outline: The version guard compares what Herald reports to the configured minimum
    Given an operator who holds the right to operate the fleet
    And the minimum Herald version is "<minimum>"
    And data planes "alpha" whose Herald reports "<reported>"
    When the operator requests an upgrade to "26.9.0" of the data planes "alpha"
    Then the request is <outcome>

    Examples:
      | reported   | minimum | outcome                                                          |
      | 26.3.0     | 26.2.0  | accepted                                                         |
      | 26.2.0     | 26.2.0  | accepted                                                         |
      | 26.1.0     | 26.2.0  | refused because "the Herald is below the minimum"                |
      | no version | 26.2.0  | refused because "the Herald reports no version"                  |
      | 26.3.0     | unset   | refused because "no minimum Herald version is configured"        |

  @spec-req-6
  Scenario: Requesting the version already under way returns the existing action
    Given an operator who holds the right to operate the fleet
    And the minimum Herald version is "26.0.0"
    And data planes "alpha" whose Herald reports "26.1.0"
    And an upgrade of "alpha" to "26.2.0" was requested
    When the operator requests an upgrade to "26.2.0" of the data planes "alpha"
    Then the existing action is returned for "alpha"
    And "alpha" has 1 upgrade action

  @spec-req-6
  Scenario: Requesting another version while one is under way is refused
    Given an operator who holds the right to operate the fleet
    And the minimum Herald version is "26.0.0"
    And data planes "alpha" whose Herald reports "26.1.0"
    And an upgrade of "alpha" to "26.2.0" was requested
    When the operator requests an upgrade to "26.3.0" of the data planes "alpha"
    Then the request is refused because "an upgrade to another version is under way"
    And "alpha" has 1 upgrade action

  @spec-req-7
  Scenario Outline: A finished upgrade no longer blocks a new request
    Given an operator who holds the right to operate the fleet
    And the minimum Herald version is "26.0.0"
    And data planes "alpha" whose Herald reports "26.1.0"
    And an upgrade of "alpha" to "26.2.0" was requested
    And the upgrade of "alpha" is <finish>
    When the operator requests an upgrade to "26.3.0" of the data planes "alpha"
    Then a new action is created for "alpha"
    And "alpha" has 2 upgrade actions

    Examples:
      | finish    |
      | published |
      | failed    |

  @spec-req-8
  Scenario: Listing returns the data planes that have an upgrade with each status
    Given an operator who holds the right to operate the fleet
    And the operator also holds the right to view the estate
    And the minimum Herald version is "26.0.0"
    And data planes "alpha, beta, gamma" whose Herald reports "26.1.0"
    And an upgrade of "alpha" to "26.2.0" was requested
    And an upgrade of "gamma" to "26.2.0" was requested
    And the upgrade of "gamma" is published
    When the operator lists the data plane upgrades
    Then the listing shows 2 data planes
    And the listing shows "alpha" with an upgrade "pending"
    And the listing shows "gamma" with an upgrade "published"
