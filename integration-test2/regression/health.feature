Feature: Performing minimal user sanity check

  Background: Background name
    * use karate with config '$regression/config'

  Scenario: Service health check should work
    Given url (serviceUrl)
    * path '/info/ready'
    When method GET
    Then status 200

  Scenario: Getting registered providers should work
    Given url (identityUrl)
    * path '/api/auth/providers'
    * method GET
    Then status 200
    * match json response contains only { "providers": ["oauth2_flow", "openid_flow"] }