// Removing annotations: input is already annotated
angular.module("MyMod").controller("MyCtrl", function($scope, $timeout) {});
myMod.controller("foo", function($scope, $timeout) {});
myMod.factory("foo", function($a, $b) {});
// no params - untouched
myMod.controller("foo", function() {});
// run
myMod.run(function($scope, $timeout) {});
