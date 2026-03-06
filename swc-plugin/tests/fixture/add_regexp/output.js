// regexp filter: only match modules whose name matches ^myMod
// should match (myMod matches ^myMod)
myMod.controller("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.service("foo", [
    "$scope",
    function($scope) {}
]);
// angular.module always matches (long-def form); otherMod does NOT match ^myMod
angular.module("MyMod").controller("foo", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
otherMod.controller("foo", function($scope) {});
