    @ignore
Feature: Oauth2 mocker server

  Scenario: pathMatches('/oauth2/token') && methodIs('post')
    * print pathParams
    * def response = karate.urlDecode(pathParams.code)
