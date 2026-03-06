// Rebuild mode: input is already annotated, should rebuild with current params
angular.module("MyMod").controller("MyCtrl", [
    "$scope",
    "$timeout",
    function($scope, $timeout) {}
]);
myMod.factory("foo", [
    "$a",
    "$b",
    function($a, $b) {}
]);
// unannotated should also be annotated
myMod.controller("bar", [
    "$scope",
    function($scope) {}
]);
