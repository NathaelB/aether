Feature: IAM settings reach the identity instance

  A customer sets the branding of an instance. The control plane stores it and
  sends the whole state as one action, Genesis writes it into the resource, and
  the operator maps it to the provider's theme. A field renamed on one side
  only breaks a scenario here.

  @spec-iam-1
  Scenario: A valid branding is stored whole and sent as one action
    Given a deployment of the organisation
    When the customer saves the branding '{"colors":{"primary":"#112233","primary_text":"#ffffff","error":"#ff0000"},"radius":6}'
    Then the change is accepted
    And the deployment stores the branding '{"colors":{"primary":"#112233","primary_text":"#ffffff","error":"#ff0000"},"radius":6}'
    And exactly one "deployment.iam_settings" action is created
    And the action payload has exactly the keys "branding", "deployment_id" and "namespace"
    And the action carries the branding '{"colors":{"primary":"#112233","primary_text":"#ffffff","error":"#ff0000"},"radius":6}'

  @spec-iam-2
  Scenario Outline: An invalid branding is refused and nothing is stored or created
    Given a deployment of the organisation
    When the customer saves the branding '<branding>'
    Then the branding is refused
    And nothing is stored and no action is created

    Examples:
      | branding |
      | {"colors":{"primary":"blue"}} |
      | {"colors":{"links":"#12345"}} |
      | {"colors":{"text":"#1234567"}} |
      | {"colors":{"error":"112233"}} |
      | {"radius":25} |
      | {"radius":-1} |
      | {"font":"serif"} |
      | {"colors":{"accent":"#112233"}} |

  @spec-iam-3
  Scenario: Clearing sends a null branding
    Given a deployment of the organisation
    And the customer has saved the branding '{"radius":6}'
    When the customer clears the branding
    Then the change is accepted
    And the deployment stores no branding
    And the latest action carries a null branding

  @spec-iam-3
  Scenario: Clearing is accepted even when the plan closes branding
    Given a deployment of the organisation
    And a plan that closes branding
    When the customer clears the branding
    Then the change is accepted
    And the latest action carries a null branding

  @spec-iam-3
  Scenario: Setting a branding is refused when the plan closes it
    Given a deployment of the organisation
    And a plan that closes branding
    When the customer saves the branding '{"radius":6}'
    Then the change is refused as permission denied
    And nothing is stored and no action is created

  @spec-iam-4
  Scenario: A caller without the right to manage instances is refused
    Given a deployment of the organisation
    And a caller who may view instances but not manage them
    When the customer saves the branding '{"radius":6}'
    Then the change is refused as permission denied
    And nothing is stored and no action is created

  @spec-iam-4
  Scenario: Another organisation's deployment is not found
    Given a deployment of another organisation
    When the customer saves the branding '{"radius":6}'
    Then the deployment is reported as not found
    And nothing is stored and no action is created

  @spec-iam-5
  Scenario: Genesis accepts the payload and writes the same colors and radius
    Given a deployment of the organisation
    And the customer has saved the branding '{"colors":{"primary":"#112233","primary_text":"#FFFFFF","links":"#0a0b0c","page_background":"#000000","widget_background":"#eeeeee","text":"#333333","error":"#ff0000"},"radius":24}'
    When Genesis applies the action the control plane created
    Then Genesis accepted the event
    And the instance written is the deployment's own
    And the resource holds the same colors and radius as the stored branding

  @spec-iam-5
  Scenario Outline: Each color reaches the resource under its camelCase name
    Given a deployment of the organisation
    And the customer has saved the branding '{"colors":{"<field>":"#123456"}}'
    When Genesis applies the action the control plane created
    Then the resource holds "#123456" at colors "<key>"

    Examples:
      | field | key |
      | primary | primary |
      | primary_text | primaryText |
      | links | links |
      | page_background | pageBackground |
      | widget_background | widgetBackground |
      | text | text |
      | error | error |

  @spec-iam-6
  Scenario Outline: Each field reaches FerrisKey under its theme key
    Given the resource's branding is '<branding>'
    When the operator maps it to the theme configuration
    Then the theme configuration is '<theme>'

    Examples:
      | branding | theme |
      | {"colors":{"primary":"#112233"}} | {"colors":{"primaryButton":"#112233"}} |
      | {"colors":{"primaryText":"#112233"}} | {"colors":{"primaryButtonLabel":"#112233"}} |
      | {"colors":{"links":"#112233"}} | {"colors":{"links":"#112233"}} |
      | {"colors":{"pageBackground":"#112233"}} | {"colors":{"pageBackground":"#112233"}} |
      | {"colors":{"widgetBackground":"#112233"}} | {"colors":{"widgetBackground":"#112233"}} |
      | {"colors":{"text":"#112233"}} | {"colors":{"bodyText":"#112233"}} |
      | {"colors":{"error":"#112233"}} | {"colors":{"error":"#112233"}} |
      | {"radius":8} | {"borders":{"widgetRadius":8,"buttonRadius":8,"inputRadius":8}} |
      | {"radius":0} | {"borders":{"widgetRadius":0,"buttonRadius":0,"inputRadius":0}} |
      | {"colors":{}} | {} |
      | {} | {} |

  @spec-iam-7
  Scenario Outline: The operator decides from the branding and the marker of what is live
    Given the resource's branding is '<branding>'
    When the operator decides with a "<marker>" marker
    Then the decision is "<decision>"

    Examples:
      | branding | marker | decision |
      | {"radius":6} | none | apply |
      | {"radius":6} | other | apply |
      | {"radius":6} | same | nothing |
      | null | other | restore the default |
      | null | none | nothing |
      | {"colors":{}} | other | restore the default |
      | {"colors":{}} | none | nothing |

  @spec-iam-8
  Scenario: A null branding clears the resource's settings
    Given a deployment of the organisation
    And the customer has cleared the branding
    When Genesis applies the action the control plane created
    Then Genesis accepted the event
    And the resource's iam is cleared

  @spec-iam-8
  Scenario: A second identical event leaves the same value
    Given a deployment of the organisation
    And the customer has saved the branding '{"colors":{"primary":"#112233"},"radius":2}'
    When Genesis applies the action the control plane created twice
    Then Genesis accepted the event
    And both writes are the same patch
