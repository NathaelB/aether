Feature: IAM features per plan

  A plan opens a closed set of IAM features. Today every plan opens every
  one, and restricting a feature to a tier is a change to one place.

  @spec-feat-1
  Scenario Outline: Every plan opens every IAM feature today
    Given an organisation on the "<plan>" plan
    When its features are listed
    Then "<feature>" is open

    Examples:
      | plan | feature |
      | free | sso_connectors |
      | free | mfa |
      | free | directory_federation |
      | free | custom_domain |
      | free | branding |
      | free | analytics |
      | free | compliance |
      | free | delegated_admin |
      | starter | sso_connectors |
      | starter | mfa |
      | starter | directory_federation |
      | starter | custom_domain |
      | starter | branding |
      | starter | analytics |
      | starter | compliance |
      | starter | delegated_admin |
      | business | sso_connectors |
      | business | mfa |
      | business | directory_federation |
      | business | custom_domain |
      | business | branding |
      | business | analytics |
      | business | compliance |
      | business | delegated_admin |
      | enterprise | sso_connectors |
      | enterprise | mfa |
      | enterprise | directory_federation |
      | enterprise | custom_domain |
      | enterprise | branding |
      | enterprise | analytics |
      | enterprise | compliance |
      | enterprise | delegated_admin |

  @spec-feat-2
  Scenario: The feature set is the eight IAM features
    When the feature set is listed
    Then the features are:
      | feature |
      | sso_connectors |
      | mfa |
      | directory_federation |
      | custom_domain |
      | branding |
      | analytics |
      | compliance |
      | delegated_admin |

  @spec-feat-3
  Scenario: A feature the plan opens names no unlocking tier
    Given an organisation on the "starter" plan
    When its features are listed
    Then "mfa" is open with no unlocking tier

  @spec-feat-4
  Scenario: The tier walk names the cheapest plan that opens a feature
    When the cheapest plan opening "branding" is asked
    Then the answer is the "free" plan
