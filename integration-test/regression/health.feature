Feature: fetching User Details

  Scenario: testing the get call for User Details
    Given url '${serviceUrl}'
    * path '/info/ready'
    When method GET
    Then status 200

  Scenario: testing the get call for User Details
    Given url '${identityUrl}'
    * path '/api/auth/providers'
    * method GET
    Then status 200
    * match json response contains only { "providers": ["oauth2_flow", "openid_flow"] }