// Removing annotations: input is already annotated

angular.module("MyMod").controller("MyCtrl", ["$scope", "$timeout", function($scope, $timeout) {}]);
myMod.controller("foo", ["$scope", "$timeout", function($scope, $timeout) {}]);
myMod.factory("foo", ["$a", "$b", function($a, $b) {}]);

// no params - untouched
myMod.controller("foo", function() {});

// run
myMod.run(["$scope", "$timeout", function($scope, $timeout) {}]);
