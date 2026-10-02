Feature: Data plane upgrade

  @spec-dpu-1
  Scenario: A valid upgrade request is accepted by the control plane and by Genesis
    When an upgrade request for components "Herald, Genesis" is built with max_unavailable 1
    Then the control plane accepts the request
    And Genesis accepts the request

  @spec-dpu-1
  Scenario Outline: An invalid upgrade request is rejected by the control plane and by Genesis
    When an upgrade request for components "<components>" is built with max_unavailable <max_unavailable>
    Then the control plane rejects the request
    And Genesis rejects the request

    Examples:
      | components | max_unavailable |
      | none       | 1               |
      | Herald     | 0               |

  @spec-dpu-2
  Scenario: Genesis creates the upgrade of a data plane from the action Herald publishes
    When Herald publishes an upgrade to "26.1.0" of Herald, Genesis and Operator
    Then Genesis holds one upgrade named after the data plane
    And the upgrade phase is "Pending"
    And the action carries no deployment id

  @spec-dpu-3
  Scenario: The same event delivered twice creates one upgrade
    Given Herald published an upgrade to "26.1.0" of Herald, Genesis and Operator
    When Herald publishes an upgrade to "26.1.0" of Herald, Genesis and Operator
    Then Genesis holds one upgrade named after the data plane
    And the creation count is 1

  @spec-dpu-4
  Scenario: A different version while an upgrade is in flight is refused
    Given Herald published an upgrade to "26.1.0" of Herald, Genesis and Operator
    When Herald publishes an upgrade to "27.0.0" of Herald, Genesis and Operator
    Then Genesis refuses the action
    And the creation count is 1
    And the upgrade targets "26.1.0"

  @spec-dpu-5
  Scenario: A rolling upgrade moves one component at a time, the operator last
    Given a data plane running "26.0.0"
    And a rolling upgrade to "26.1.0" of Herald, Genesis and Operator was requested
    When the operator reconciles until the upgrade settles
    Then the upgrade is "Completed" at version "26.1.0"
    And the components were patched in the order "Herald, Genesis, Operator"
    And every component runs "26.1.0"

  @spec-dpu-6
  Scenario: A component that never becomes ready rolls the upgrade back
    Given a data plane running "26.0.0"
    And a rolling upgrade to "26.1.0" of Herald, Genesis and Operator was requested
    And Genesis never becomes ready on the target version
    When the operator reconciles until the upgrade settles
    Then the upgrade ends "RolledBack"
    And the components were restored in the order "Genesis, Herald"
    And every component runs "26.0.0"

  @spec-dpu-7
  Scenario: A restarted operator resumes from the recorded status
    Given a data plane running "26.0.0"
    And a rolling upgrade to "26.1.0" of Herald, Genesis and Operator was requested
    And the operator recorded Herald as upgraded and Genesis as upgrading
    When the operator reconciles until the upgrade settles
    Then the upgrade is "Completed" at version "26.1.0"
    And the components were patched in the order "Operator"

  @spec-dpu-8
  Scenario: A canary upgrade fails without touching any deployment
    Given a data plane running "26.0.0"
    And a canary upgrade to "26.1.0" of Herald, Genesis and Operator was requested
    When the operator reconciles until the upgrade settles
    Then the upgrade is "Failed" with reason "CanaryNotSupported"
    And no deployment was touched

  @spec-dpu-9
  Scenario Outline: A finished upgrade at the same version is requested again
    Given Herald published an upgrade to "26.1.0" of Herald, Genesis and Operator
    And the upgrade finished as "<phase>"
    When Herald publishes an upgrade to "26.1.0" of Herald, Genesis and Operator
    Then the replacement count is <replacements>

    Examples:
      | phase      | replacements |
      | Failed     | 1            |
      | RolledBack | 1            |
      | Completed  | 0            |

  @spec-dpu-10
  Scenario Outline: The broker routes a routing key to Genesis only when its queue is bound to it
    When Genesis' queue is asked about the routing key "<routing_key>"
    Then the routing key is <binding>

    Examples:
      | routing_key        | binding |
      | dataplane.upgrade  | bound   |
      | deployment.create  | bound   |
      | outcome.deployment | unbound |
