// regexp filter: only match modules whose name matches ^myMod

// should match (myMod matches ^myMod)
myMod.controller("foo", function($scope, $timeout) {});
myMod.service("foo", function($scope) {});

// angular.module always matches (long-def form); otherMod does NOT match ^myMod
angular.module("MyMod").controller("foo", function($scope, $timeout) {});
otherMod.controller("foo", function($scope) {});
