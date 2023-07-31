Feature: Auth Status

  Scenario: Get providers
    Given a GET request to the API at "/auth/providers"
    When the response is received
    Then the response should have a status 200
    And the response should have a json like
      """
      {
        "providers": [
          "openid_flow",
          "oauth2_flow"
        ]
      }
      """
