// Case 1: block comment on preceding line, function declaration
/* @ngInject */ function case1($scope, $http) {}
case1.$inject = [
    "$scope",
    "$http"
];
// Case 2: block comment on preceding line, var + function expression
/* @ngInject */ var case2 = function($scope) {};
case2.$inject = [
    "$scope"
];
// Case 3: block comment on preceding line, var + arrow
/* @ngInject */ var case3 = ($a, $b)=>{};
case3.$inject = [
    "$a",
    "$b"
];
// Case 4: inline before function expression in var
var case4 = /* @ngInject */ function($scope) {};
case4.$inject = [
    "$scope"
];
// Case 5: inline before function arg in Angular call (already a suspect, but should not double-wrap)
myMod.controller("c5", [
    "$scope",
    /* @ngInject */ function($scope) {}
]);
// Case 6: block comment inline on same line as function declaration
/* @ngInject */ function case6($scope) {}
case6.$inject = [
    "$scope"
];
