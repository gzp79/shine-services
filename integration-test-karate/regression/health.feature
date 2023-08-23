Feature: fetching User Details

  Scenario: testing the get call for User Details
    Given url karate.properties['utils'].serviceUrl
    * path '/info/ready'
    * method GET
    Then status 200

  Scenario: testing the get call for User Details
    Given url karate.properties['utils'].identityUrl
    * path '/api/auth/providers'
    * method GET
    Then status 200
    * def expectedProviders = ["oauth2_flow", "openid_flow"]
    * match response == { "providers": #(^^expectedProviders) }