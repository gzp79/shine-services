function fn() {
    karate.properties['serviceDomain'] = 'test.scytta.com';
    karate.properties['serviceUrl'] = 'http://' + karate.properties['serviceDomain'];
    karate.properties['identityUrl'] = karate.properties['serviceUrl'] + '/identity';
}
