// Arrow function annotations

angular.module("MyMod").controller("MyCtrl", ($scope, $timeout) => {});
myMod.controller("foo", ($scope, $timeout) => {});
myMod.factory("foo", ($a, $b) => {});
myMod.run(($scope, $timeout) => {});
myMod.config(($scope, $timeout) => {});

// no dependencies
myMod.controller("foo", () => {});

// provider with arrow $get
myMod.provider("foo", function($scope) {
    this.$get = ($scope, $timeout) => { bar; };
});

// directive return object with arrow
myMod.directive("foo", $scope => ({
    controller: ($scope, $timeout) => { bar; }
}));
